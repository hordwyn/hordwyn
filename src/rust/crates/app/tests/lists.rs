mod common;

use app::AppError;
use domain::list::Visibility;
use testkit::TestEnv;

#[tokio::test]
async fn list_lifecycle() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let (_, other) = common::user(&env, "bob").await;
    let a = common::item(&env.app, "movie", "A").await;
    let b = common::item(&env.app, "book", "B").await;

    let list = env
        .app
        .create_list(me, "Favourites", None, Visibility::Private)
        .await
        .unwrap();
    env.app.add_to_list(me, list.id, a.id, None).await.unwrap();
    env.app
        .add_to_list(me, list.id, b.id, Some("gift idea".into()))
        .await
        .unwrap();
    env.app.add_to_list(me, list.id, a.id, None).await.unwrap();
    env.app.move_in_list(me, list.id, b.id, 0).await.unwrap();
    let l = env.app.list(me, list.id).await.unwrap();
    assert_eq!(
        l.items.iter().map(|i| i.item_id).collect::<Vec<_>>(),
        vec![b.id, a.id]
    );

    let err = env
        .app
        .add_to_list(other, list.id, a.id, None)
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::Forbidden), "{err:?}");
    assert!(matches!(
        env.app.list(other, list.id).await,
        Err(AppError::Forbidden)
    ));

    env.app
        .update_list(
            me,
            list.id,
            Some("Best".into()),
            None,
            Some(Visibility::Instance),
        )
        .await
        .unwrap();
    assert_eq!(env.app.list(other, list.id).await.unwrap().name, "Best");

    env.app.remove_from_list(me, list.id, a.id).await.unwrap();
    env.app.delete_list(me, list.id).await.unwrap();
    assert!(matches!(
        env.app.list(me, list.id).await,
        Err(AppError::NotFound(_))
    ));
}

#[tokio::test]
async fn link_shared_lists_are_read_by_token_only() {
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let (_, other) = common::user(&env, "bob").await;
    let list = env
        .app
        .create_list(me, "Picks", None, Visibility::Private)
        .await
        .unwrap();
    let token = env.app.share_list(me, list.id).await.unwrap();
    assert_ne!(token, "");
    assert_eq!(env.app.list_by_token(&token).await.unwrap().id, list.id);
    // the token, not the id, grants access
    assert!(matches!(
        env.app.list(other, list.id).await,
        Err(AppError::Forbidden)
    ));
    assert!(matches!(
        env.app.share_list(other, list.id).await,
        Err(AppError::Forbidden)
    ));
    // sharing again rotates the token
    let rotated = env.app.share_list(me, list.id).await.unwrap();
    assert_ne!(rotated, token);
    assert!(matches!(
        env.app.list_by_token(&token).await,
        Err(AppError::NotFound(_))
    ));
    // back to private revokes the link
    env.app
        .update_list(me, list.id, None, None, Some(Visibility::Private))
        .await
        .unwrap();
    assert!(matches!(
        env.app.list_by_token(&rotated).await,
        Err(AppError::NotFound(_))
    ));
}

/// WP3-6 (design.md 5.10): the share dialog's three choices. "Anyone with the link" makes a
/// link and keeps it while it stays chosen; asking for a new link replaces it; "Private"
/// revokes it, so the old link finds nothing.
#[tokio::test]
async fn visibility_transitions_make_keep_replace_and_revoke_the_link() {
    use domain::list::VisibilityChoice;
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let (_, other) = common::user(&env, "bob").await;
    let list = env
        .app
        .create_list(me, "Picks", None, Visibility::Private)
        .await
        .unwrap();

    let l = env
        .app
        .set_list_visibility(me, list.id, VisibilityChoice::Instance, false)
        .await
        .unwrap();
    assert_eq!(l.visibility, Visibility::Instance);
    assert_eq!(env.app.list(other, list.id).await.unwrap().id, list.id);

    let l = env
        .app
        .set_list_visibility(me, list.id, VisibilityChoice::Link, false)
        .await
        .unwrap();
    let Visibility::Link { token: first } = l.visibility.clone() else {
        panic!("{l:?}");
    };
    assert!(matches!(
        env.app.list(other, list.id).await,
        Err(AppError::Forbidden)
    ));
    // saving the same choice again keeps the link people already have
    let l = env
        .app
        .set_list_visibility(me, list.id, VisibilityChoice::Link, false)
        .await
        .unwrap();
    assert_eq!(
        l.visibility,
        Visibility::Link {
            token: first.clone()
        }
    );
    // a new link replaces the old one
    let l = env
        .app
        .set_list_visibility(me, list.id, VisibilityChoice::Link, true)
        .await
        .unwrap();
    let Visibility::Link { token: second } = l.visibility.clone() else {
        panic!("{l:?}");
    };
    assert_ne!(first, second);
    assert!(matches!(
        env.app.list_by_token(&first).await,
        Err(AppError::NotFound(_))
    ));
    // private again revokes it
    env.app
        .set_list_visibility(me, list.id, VisibilityChoice::Private, false)
        .await
        .unwrap();
    assert!(matches!(
        env.app.list_by_token(&second).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        env.app
            .set_list_visibility(other, list.id, VisibilityChoice::Instance, false)
            .await,
        Err(AppError::Forbidden)
    ));
}

/// "Shared with me" holds the instance-visible lists of linked people; "On this instance"
/// every instance-visible list of others; private and link-shared lists are in neither.
#[tokio::test]
async fn shared_with_me_and_on_this_instance() {
    use domain::list::VisibilityChoice;
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let (bob_user, bob) = common::user(&env, "bob").await;
    let (_, carl) = common::user(&env, "carl").await;
    let c = env.app.create_companion(me, "Bob").await.unwrap();
    let req = env.app.request_link(me, c.id, bob_user.id).await.unwrap();
    env.app.accept_link(bob, req.id).await.unwrap();

    let make = |who, name: &'static str, v| {
        let app = &env.app;
        async move {
            let l = app
                .create_list(who, name, None, Visibility::Private)
                .await
                .unwrap();
            app.set_list_visibility(who, l.id, v, false).await.unwrap()
        }
    };
    let bobs = make(bob, "Bob's picks", VisibilityChoice::Instance).await;
    let carls = make(carl, "Carl's picks", VisibilityChoice::Instance).await;
    make(carl, "Carl's secret", VisibilityChoice::Private).await;
    make(bob, "Bob's link", VisibilityChoice::Link).await;
    make(me, "Mine", VisibilityChoice::Instance).await;

    let ids = |ls: Vec<domain::list::List>| ls.into_iter().map(|l| l.id).collect::<Vec<_>>();
    assert_eq!(
        ids(env.app.lists_shared_with_me(me).await.unwrap()),
        vec![bobs.id]
    );
    let mut here = ids(env.app.lists_on_instance(me).await.unwrap());
    here.sort();
    let mut want = vec![bobs.id, carls.id];
    want.sort();
    assert_eq!(here, want);
    // Bob sees Alice's list as shared with him: linking works both ways
    let names: Vec<String> = env
        .app
        .lists_shared_with_me(bob)
        .await
        .unwrap()
        .into_iter()
        .map(|l| l.name)
        .collect();
    assert_eq!(names, vec!["Mine".to_string()]);
}

/// "Add all to my library" adds what is not there yet and counts both.
#[tokio::test]
async fn add_all_to_my_library() {
    use domain::list::VisibilityChoice;
    let env = TestEnv::new();
    let (_, me) = common::user(&env, "alice").await;
    let (_, other) = common::user(&env, "bob").await;
    let a = common::item(&env.app, "movie", "A").await;
    let b = common::item(&env.app, "book", "B").await;
    let list = env
        .app
        .create_list(me, "Picks", None, Visibility::Private)
        .await
        .unwrap();
    env.app.add_to_list(me, list.id, a.id, None).await.unwrap();
    env.app.add_to_list(me, list.id, b.id, None).await.unwrap();
    assert!(matches!(
        env.app.add_list_to_library(other, list.id).await,
        Err(AppError::Forbidden)
    ));
    env.app
        .set_list_visibility(me, list.id, VisibilityChoice::Instance, false)
        .await
        .unwrap();
    env.app.add_to_library(other, a.id, None).await.unwrap();
    assert_eq!(
        env.app.add_list_to_library(other, list.id).await.unwrap(),
        (1, 1)
    );
    assert_eq!(
        env.app.add_list_to_library(other, list.id).await.unwrap(),
        (0, 2)
    );
    let l = env
        .app
        .set_list_visibility(me, list.id, VisibilityChoice::Link, false)
        .await
        .unwrap();
    let Visibility::Link { token } = l.visibility else {
        panic!("not a link");
    };
    let (_, carl) = common::user(&env, "carl").await;
    assert_eq!(
        env.app
            .add_shared_list_to_library(carl, &token)
            .await
            .unwrap(),
        (2, 0)
    );
}
