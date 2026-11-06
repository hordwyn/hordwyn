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
