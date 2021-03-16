use crate::model;
use crate::server::{auth, Result, ServerError};
use futures::TryStreamExt;
use jq_rs;
use regex::Regex;
use sqlite::SqlitePoolOptions;
use sqlx::{migrate::MigrateDatabase, sqlite, Row};
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
) -> Result<super::Creds> {
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
) -> Result<Vec<UserList>> {
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
) -> Result<super::Creds> {
    let new = auth::credentials::generate_credentials().await?;
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
        client_secret: new.creds.client_secret.clone(),
        scope: authscopes.scope,
    };
    Ok(out)
}

pub(super) async fn delete_api_user(uuid: &str, db: &sqlx::SqlitePool) -> Result<()> {
    let deleted = sqlx::query!("DELETE FROM credentials WHERE client_id = ?", uuid)
        .execute(db)
        .await?
        .rows_affected();

    if deleted > 0 {
        return Ok(());
    }
    Err(ServerError::NoRecordDeleted)
}

pub(crate) async fn get_all_academic_sessions(
    db: &sqlx::SqlitePool,
    params: crate::server::params::Parameters,
) -> Result<Vec<model::AcademicSession>> {
    let rows = sqlx::query_as!(
        model::AcademicSession,
        r#"
        SELECT json_extract(data, '$.sourcedId') as "sourced_id!: String"
            ,json_extract(data, '$.status') as "status!: String"
            ,json_extract(data, '$.year') as "year?: String"
            FROM academicSessions
            ORDER BY sourcedId
            LIMIT ?
            OFFSET ?
        "#,
        params.limit,
        params.offset
    )
    .fetch_all(db)
    .await?;

    // fields
    let mut fields = Vec::new();
    for f in params.fields.split(',') {
        fields.push(format!("{}: .{}", f, f));
    }

    //filter
    let rlogic = Regex::new(r" (AND|OR) ").unwrap();
    let mut logicals: Vec<String> = Vec::new();
    for cap in rlogic.captures_iter(&params.filter) {
        if &cap[1] == "AND" {
            logicals.push("and".to_string());
        } else {
            logicals.push("or".to_string());
        }
    }

    let raw_filters: Vec<&str> = rlogic.split(&params.filter).collect();
    let mut filters: Vec<String> = Vec::new();
    for raw in raw_filters {
        let rfilter = Regex::new(r"(\w*)(!=|>=|<=|>|<|=|~)'(.*)'").unwrap();
        for cap in rfilter.captures_iter(raw) {
            let mut predicate = &cap[2];
            if predicate == "=" {
                predicate = "==";
            };
            let filter = format!(".{} {} \"{}\"", &cap[1], predicate, &cap[3].trim());
            log::debug!("filter: {}", filter);
            filters.push(filter);
        }
    }

    let mut filter_builder: Vec<String> = Vec::new();
    filter_builder.push(filters.pop().unwrap());
    for _ in 0..logicals.len() {
        filter_builder.push(logicals.pop().unwrap());
        filter_builder.push(filters.pop().unwrap());
    }

    // sort
    let q = format!(
        "[ .[] | {{ {} }} | select({}) ] | sort_by(.{})",
        &fields.join(","),
        &filter_builder.join(" "),
        &params.sort
    );

    log::debug!("JQ filter: {}", q);
    let output = jq_rs::run(&q, &json!(rows).to_string()).unwrap();
    let o = serde_json::from_str(&output).unwrap();
    Ok(o)
}

pub(crate) async fn put_academic_sessions(
    data: Vec<model::AcademicSession>,
    db: &sqlx::SqlitePool,
) -> Result<()> {
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
pub(super) async fn init(path: &str) -> Result<sqlx::Pool<sqlx::Sqlite>> {
    init_db(path).await?;
    let pool = connect(path).await?;
    init_schema(&pool).await?;
    Ok(pool)
}

pub(super) async fn init_db(path: &str) -> Result<()> {
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

pub(super) async fn init_schema(pool: &sqlx::SqlitePool) -> Result<()> {
    let mut t = pool.begin().await?;
    sqlx::query_file!("db/schema.sql").execute(&mut t).await?;
    sqlx::query_file!("db/init.sql").execute(&mut t).await?;
    t.commit().await?;
    Ok(())
}

pub(super) async fn connect(path: &str) -> Result<sqlx::Pool<sqlx::Sqlite>> {
    log::info!("connecting to database...");
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(path)
        .await?;
    Ok(pool)
}
