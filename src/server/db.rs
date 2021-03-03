use crate::model;
use crate::server::auth;
use sqlite::SqlitePoolOptions;
use sqlx::{migrate::MigrateDatabase, sqlite};
use tide::prelude::*;

#[derive(Serialize)]
pub(super) struct UserList {
    tag: String,
    client_id: String,
    scope: String,
}

pub(super) async fn get_api_creds(
    client_id: &String,
    db: &sqlx::SqlitePool,
) -> sqlx::Result<super::Creds> {
    let res = sqlx::query_as!(
        super::Creds,
        r#"
        SELECT
            c.client_id
            , c.client_secret
            , group_concat(s.scope,' ') AS "scope!: String"
        FROM
            credentials c
            INNER JOIN credential_scopes cs ON c.id = cs.credential_id
            INNER JOIN scopes s ON cs.scope_id = s.id
        WHERE
            c.client_id = ?
            AND scope IS NOT NULL
        GROUP BY 
            c.client_id
        "#,
        client_id
    )
    .fetch_one(db)
    .await?;
    Ok(res)
}

pub(super) async fn get_api_users(
    fcol: String,
    fval: String,
    db: &sqlx::SqlitePool,
) -> sqlx::Result<Vec<UserList>> {
    let rows = sqlx::query_as!(
        UserList,
        r#"
        SELECT
            c.client_id
            , c.tag
            , group_concat(s.scope,' ') AS "scope!: String"
        FROM
            credentials c
            INNER JOIN credential_scopes cs ON c.id = cs.credential_id
            INNER JOIN scopes s ON cs.scope_id = s.id
        WHERE
            ? = ?
            AND scope IS NOT NULL
        GROUP BY 
            c.client_id
        "#,
        fcol,
        fval,
    )
    .fetch_all(db)
    .await?;

    Ok(rows)
}

#[derive(Deserialize)]
pub(super) struct CreateApiUser {
    tag: String,
    scope: String,
}

pub(super) async fn create_api_user(
    user: CreateApiUser,
    db: &sqlx::SqlitePool,
) -> tide::Result<super::Creds> {
    let new = auth::generate_credentials().await?;
    let mut t = db.begin().await?;
    sqlx::query!(
        "INSERT INTO credentials(client_id, client_secret, tag) VALUES (?, ?, ?)",
        new.creds.client_id,
        new.encrypt,
        user.tag,
    )
    .execute(&mut t)
    .await?;
    for scope in user.scope.split(' ') {
        sqlx::query!(
            "INSERT OR IGNORE INTO credential_scopes (credential_id, scope_id) VALUES (
            (SELECT id FROM credentials WHERE client_id = ?),
            (SELECT id FROM scopes WHERE scope = ?)
            )",
            new.creds.client_id,
            scope,
        )
        .execute(&mut t)
        .await?;
    }
    t.commit().await?;
    let authscopes = get_api_creds(&new.creds.client_id, db).await?;
    let out = super::Creds {
        client_id: new.creds.client_id.clone(),
        client_secret: new.creds.client_id.clone(),
        scope: authscopes.scope,
    };
    Ok(out)
}

pub(super) async fn delete_api_user(uuid: &str, db: &sqlx::SqlitePool) -> sqlx::Result<bool> {
    let deleted = sqlx::query!("DELETE FROM credentials WHERE client_id = ?", uuid)
        .execute(db)
        .await?
        .rows_affected();

    if deleted > 0 {
        return Ok(true);
    }
    Ok(false)
}

pub(crate) async fn get_all_academic_sessions(
    db: &sqlx::SqlitePool,
) -> sqlx::Result<Vec<model::AcademicSession>> {
    let rows = sqlx::query!("SELECT json(data) as data FROM academicSessions")
        .fetch_all(db)
        .await?;
    let mut vs: Vec<model::AcademicSession> = Vec::new();
    for row in rows.into_iter() {
        if let Some(d) = row.data {
            let v = serde_json::from_str(&d).unwrap(); // TODO: custom error handler
            &vs.push(v);
        }
    }
    Ok(vs)
}

pub(crate) async fn put_academic_sessions(
    data: Vec<model::AcademicSession>,
    db: &sqlx::SqlitePool,
) -> sqlx::Result<()> {
    let mut t = db.begin().await?;
    for i in data.iter() {
        let json = json!(i).to_string();
        sqlx::query!(
            r#"INSERT INTO academicSessions(sourcedId, data)
            VALUES(?, json(?))
            ON CONFLICT(sourcedId)
            DO UPDATE SET sourcedId=excluded.sourcedId, data=excluded.data"#,
            i.sourced_id,
            json,
        )
        .execute(&mut t)
        .await?;
    }
    t.commit().await?;
    Ok(())
}

pub(super) async fn init(path: &str) -> sqlx::Result<()> {
    log::info!("seeking database...");
    let exist = sqlx::Sqlite::database_exists(path).await?;
    if exist {
        log::info!("found existing database");
    } else {
        log::info!("no existing database, creating...");
        sqlx::Sqlite::create_database(path).await?;
    };
    Ok(())
}

pub(super) async fn init_schema(pool: &sqlx::SqlitePool) -> sqlx::Result<()> {
    let mut t = pool.begin().await?;
    sqlx::query_file!("db/schema.sql").execute(&mut t).await?;
    sqlx::query_file!("db/init.sql").execute(&mut t).await?;
    t.commit().await?;
    Ok(())
}

pub(super) async fn connect(path: &str) -> sqlx::Result<sqlx::Pool<sqlx::Sqlite>> {
    log::info!("connecting to database...");
    return SqlitePoolOptions::new()
        .max_connections(1)
        .connect(path)
        .await;
}
