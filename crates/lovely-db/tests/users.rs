use lovely_db::users::*;
use lovely_test_support::PgTestContainer;

#[tokio::test]
#[ignore = "requires Docker"]
async fn create_and_find_user_by_username() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();

    let user = create_user(
        &pool,
        NewUser {
            username: "alice".into(),
            email: Some("alice@example.com".into()),
            password_hash: Some("$argon2id$...".into()),
        },
    )
    .await
    .unwrap();
    assert_eq!(user.username, "alice");

    let found = find_user_by_username(&pool, "alice")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.id, user.id);

    let by_id = find_user_by_id(&pool, user.id).await.unwrap().unwrap();
    assert_eq!(by_id.username, "alice");
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn username_uniqueness_enforced() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();

    create_user(
        &pool,
        NewUser {
            username: "alice".into(),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let err = create_user(
        &pool,
        NewUser {
            username: "alice".into(),
            ..Default::default()
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(err, lovely_db::DbError::Conflict(_)));
}

#[tokio::test]
#[ignore = "requires Docker"]
async fn email_optional_and_unique_when_present() {
    let pg = PgTestContainer::start().await.unwrap();
    let pool = pg.fresh_db().await.unwrap();

    create_user(
        &pool,
        NewUser {
            username: "alice".into(),
            email: Some("a@example.com".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap();
    create_user(
        &pool,
        NewUser {
            username: "bob".into(),
            email: None,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let err = create_user(
        &pool,
        NewUser {
            username: "carol".into(),
            email: Some("a@example.com".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(err, lovely_db::DbError::Conflict(_)));
}
