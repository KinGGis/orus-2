use std::sync::Arc;

use async_trait::async_trait;
use diesel::prelude::*;

use crate::db::{get_connection, DbPool};
use crate::errors::StorageError;
use crate::schema::wf_platforms;
use crate::schema::wf_platforms::dsl::*;
use wealthfolio_connect::broker::PlatformRepositoryTrait;
use wealthfolio_connect::Platform as ConnectPlatform;
use wealthfolio_core::errors::Result;

#[derive(
    Queryable,
    Identifiable,
    Insertable,
    AsChangeset,
    Selectable,
    PartialEq,
    Debug,
    Clone,
)]
#[diesel(table_name = crate::schema::wf_platforms)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct PlatformDB {
    pub id: String,
    pub name: Option<String>,
    pub url: String,
    pub external_id: Option<String>,
    pub kind: String,
    pub website_url: Option<String>,
    pub logo_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Platform {
    pub id: String,
    pub name: Option<String>,
    pub url: String,
    pub external_id: Option<String>,
    pub kind: String,
    pub website_url: Option<String>,
    pub logo_url: Option<String>,
}

impl From<PlatformDB> for Platform {
    fn from(db: PlatformDB) -> Self {
        Self {
            id: db.id,
            name: db.name,
            url: db.url,
            external_id: db.external_id,
            kind: db.kind,
            website_url: db.website_url,
            logo_url: db.logo_url,
        }
    }
}

impl From<Platform> for PlatformDB {
    fn from(platform: Platform) -> Self {
        Self {
            id: platform.id,
            name: platform.name,
            url: platform.url,
            external_id: platform.external_id,
            kind: platform.kind,
            website_url: platform.website_url,
            logo_url: platform.logo_url,
        }
    }
}

impl From<Platform> for ConnectPlatform {
    fn from(value: Platform) -> Self {
        Self {
            id: value.id,
            name: value.name,
            url: value.url,
            external_id: value.external_id,
            kind: value.kind,
            website_url: value.website_url,
            logo_url: value.logo_url,
        }
    }
}

impl From<ConnectPlatform> for Platform {
    fn from(value: ConnectPlatform) -> Self {
        Self {
            id: value.id,
            name: value.name,
            url: value.url,
            external_id: value.external_id,
            kind: value.kind,
            website_url: value.website_url,
            logo_url: value.logo_url,
        }
    }
}

pub struct PlatformRepository {
    pool: Arc<DbPool>,
}

impl PlatformRepository {
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }

    pub fn get_by_id_impl(&self, platform_id: &str) -> Result<Option<Platform>> {
        let mut conn = get_connection(&self.pool)?;

        let result = wf_platforms
            .select(PlatformDB::as_select())
            .find(platform_id)
            .first::<PlatformDB>(&mut conn)
            .optional()
            .map_err(StorageError::from)?;

        Ok(result.map(Platform::from))
    }

    pub fn get_by_external_id_impl(&self, ext_id: &str) -> Result<Option<Platform>> {
        let mut conn = get_connection(&self.pool)?;

        let result = wf_platforms
            .select(PlatformDB::as_select())
            .filter(external_id.eq(ext_id))
            .first::<PlatformDB>(&mut conn)
            .optional()
            .map_err(StorageError::from)?;

        Ok(result.map(Platform::from))
    }

    pub fn list_impl(&self) -> Result<Vec<Platform>> {
        let mut conn = get_connection(&self.pool)?;

        let results = wf_platforms
            .select(PlatformDB::as_select())
            .order(name.asc())
            .load::<PlatformDB>(&mut conn)
            .map_err(StorageError::from)?;

        Ok(results.into_iter().map(Platform::from).collect())
    }

    pub async fn upsert_impl(&self, platform: Platform) -> Result<Platform> {
        let platform_db: PlatformDB = platform.into();
        let mut conn = get_connection(&self.pool)?;

        diesel::insert_into(wf_platforms::table)
            .values(&platform_db)
            .on_conflict(wf_platforms::id)
            .do_update()
            .set((
                wf_platforms::name.eq(&platform_db.name),
                wf_platforms::url.eq(&platform_db.url),
                wf_platforms::external_id.eq(&platform_db.external_id),
                wf_platforms::kind.eq(&platform_db.kind),
                wf_platforms::website_url.eq(&platform_db.website_url),
                wf_platforms::logo_url.eq(&platform_db.logo_url),
            ))
            .execute(&mut conn)
            .map_err(StorageError::from)?;

        Ok(Platform::from(platform_db))
    }

    pub async fn delete_impl(&self, platform_id: &str) -> Result<usize> {
        let mut conn = get_connection(&self.pool)?;

        diesel::delete(wf_platforms.find(platform_id))
            .execute(&mut conn)
            .map_err(StorageError::from)
            .map_err(Into::into)
    }
}

#[async_trait]
impl PlatformRepositoryTrait for PlatformRepository {
    fn get_by_id(&self, platform_id: &str) -> Result<Option<ConnectPlatform>> {
        self.get_by_id_impl(platform_id).map(|item| item.map(Into::into))
    }

    fn get_by_external_id(&self, ext_id: &str) -> Result<Option<ConnectPlatform>> {
        self.get_by_external_id_impl(ext_id)
            .map(|item| item.map(Into::into))
    }

    fn list(&self) -> Result<Vec<ConnectPlatform>> {
        self.list_impl()
            .map(|items| items.into_iter().map(Into::into).collect())
    }

    async fn upsert(&self, platform: ConnectPlatform) -> Result<ConnectPlatform> {
        self.upsert_impl(platform.into()).await.map(Into::into)
    }

    async fn delete(&self, platform_id: &str) -> Result<usize> {
        self.delete_impl(platform_id).await
    }
}