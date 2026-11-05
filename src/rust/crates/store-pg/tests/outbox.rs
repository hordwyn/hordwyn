//! Outbox and change feed are written in the same transaction as the change.

mod common;

use domain::user::Role;

#[tokio::test]
async fn successful_write_adds_outbox_and_change_rows() {
    let Some(env) = common::env().await else {
        return;
    };
    let (u, me) = common::user(&env.app, "alice").await;
    let before = common::count(&env.store, "SELECT count(*) FROM outbox").await;
    env.app.create_companion(me, "Anna").await.unwrap();
    assert_eq!(
        common::count(&env.store, "SELECT count(*) FROM outbox").await,
        before + 1
    );

    let changes = env.store.changes_since(u.id, 0, 100).await.unwrap();
    let seqs: Vec<i64> = changes.iter().map(|c| c.seq).collect();
    assert_eq!(
        seqs,
        (1..=i64::try_from(seqs.len()).unwrap()).collect::<Vec<_>>()
    );
    assert_eq!(changes.last().unwrap().entity, "companion");
    let tail = env
        .store
        .changes_since(u.id, seqs[seqs.len() - 2], 100)
        .await
        .unwrap();
    assert_eq!(tail.len(), 1);
}

#[tokio::test]
async fn rolled_back_write_leaves_no_outbox_row() {
    let Some(env) = common::env().await else {
        return;
    };
    let (_, me) = common::user(&env.app, "alice").await;
    // Make the last statement of every write (the change-feed insert, which runs after the
    // entity row and the outbox row) fail, so the transaction must roll back.
    sqlx::raw_sql(
        "CREATE FUNCTION fail_change() RETURNS trigger LANGUAGE plpgsql AS
           $$ BEGIN RAISE EXCEPTION 'injected failure'; END $$;
         CREATE TRIGGER fail_change BEFORE INSERT ON user_changes
           FOR EACH ROW EXECUTE FUNCTION fail_change();",
    )
    .execute(env.store.pool())
    .await
    .unwrap();
    let outbox = common::count(&env.store, "SELECT count(*) FROM outbox").await;
    let companions = common::count(&env.store, "SELECT count(*) FROM companions").await;

    assert!(env.app.create_companion(me, "Anna").await.is_err());
    assert!(
        env.app
            .register_user("bob", "Bob", None, Role::Member)
            .await
            .is_err()
    );

    assert_eq!(
        common::count(&env.store, "SELECT count(*) FROM outbox").await,
        outbox
    );
    assert_eq!(
        common::count(&env.store, "SELECT count(*) FROM companions").await,
        companions
    );
    assert_eq!(
        common::count(&env.store, "SELECT count(*) FROM users").await,
        1
    );
}
