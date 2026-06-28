use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use diesel::prelude::*;
use uuid::Uuid;

use crate::db::{get_connection, DbPool};
use crate::errors::StorageError;
use crate::schema::{wf_asset_taxonomy_assignments, wf_taxonomies, wf_taxonomy_categories};
use wealthfolio_core::taxonomies::{
    AssetTaxonomyAssignment, Category, NewAssetTaxonomyAssignment, NewCategory, NewTaxonomy,
    Taxonomy, TaxonomyRepositoryTrait, TaxonomyWithCategories,
};
use wealthfolio_core::Result;

#[derive(Queryable, Identifiable, AsChangeset, Selectable, Insertable, Debug, Clone)]
#[diesel(table_name = wf_taxonomies)]
#[diesel(check_for_backend(diesel::pg::Pg))]
struct TaxonomyDB {
    id: Uuid,
    name: String,
    color: String,
    description: Option<String>,
    is_system: bool,
    is_single_select: bool,
    sort_order: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Queryable, Identifiable, AsChangeset, Selectable, Insertable, Debug, Clone)]
#[diesel(table_name = wf_taxonomy_categories)]
#[diesel(primary_key(id, taxonomy_id))]
#[diesel(check_for_backend(diesel::pg::Pg))]
struct CategoryDB {
    id: Uuid,
    taxonomy_id: Uuid,
    parent_id: Option<Uuid>,
    name: String,
    key: String,
    color: String,
    description: Option<String>,
    sort_order: i32,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Queryable, Identifiable, AsChangeset, Selectable, Insertable, Debug, Clone)]
#[diesel(table_name = wf_asset_taxonomy_assignments)]
#[diesel(check_for_backend(diesel::pg::Pg))]
struct AssetTaxonomyAssignmentDB {
    id: Uuid,
    asset_id: Uuid,
    taxonomy_id: Uuid,
    category_id: Uuid,
    weight: i32,
    source: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

fn parse_uuid(value: &str, field: &str) -> Result<Uuid> {
    Uuid::parse_str(value).map_err(|err| {
        wealthfolio_core::Error::Validation(wealthfolio_core::errors::ValidationError::InvalidInput(
            format!("Invalid {field} UUID: {err}"),
        ))
    })
}

impl From<TaxonomyDB> for Taxonomy {
    fn from(db: TaxonomyDB) -> Self {
        Self {
            id: db.id.to_string(),
            name: db.name,
            color: db.color,
            description: db.description,
            is_system: db.is_system,
            is_single_select: db.is_single_select,
            sort_order: db.sort_order,
            created_at: db.created_at.naive_utc(),
            updated_at: db.updated_at.naive_utc(),
        }
    }
}

impl From<CategoryDB> for Category {
    fn from(db: CategoryDB) -> Self {
        Self {
            id: db.id.to_string(),
            taxonomy_id: db.taxonomy_id.to_string(),
            parent_id: db.parent_id.map(|value| value.to_string()),
            name: db.name,
            key: db.key,
            color: db.color,
            description: db.description,
            sort_order: db.sort_order,
            created_at: db.created_at.naive_utc(),
            updated_at: db.updated_at.naive_utc(),
        }
    }
}

impl From<AssetTaxonomyAssignmentDB> for AssetTaxonomyAssignment {
    fn from(db: AssetTaxonomyAssignmentDB) -> Self {
        Self {
            id: db.id.to_string(),
            asset_id: db.asset_id.to_string(),
            taxonomy_id: db.taxonomy_id.to_string(),
            category_id: db.category_id.to_string(),
            weight: db.weight,
            source: db.source,
            created_at: db.created_at.naive_utc(),
            updated_at: db.updated_at.naive_utc(),
        }
    }
}

pub struct TaxonomyRepository {
    pool: Arc<DbPool>,
}

impl TaxonomyRepository {
    pub fn new(pool: Arc<DbPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TaxonomyRepositoryTrait for TaxonomyRepository {
    fn get_taxonomies(&self) -> Result<Vec<Taxonomy>> {
        let mut conn = get_connection(&self.pool)?;
        let results = wf_taxonomies::table
            .order(wf_taxonomies::sort_order.asc())
            .select(TaxonomyDB::as_select())
            .load::<TaxonomyDB>(&mut conn)
            .map_err(StorageError::from)?;
        Ok(results.into_iter().map(Into::into).collect())
    }

    fn get_taxonomy(&self, id: &str) -> Result<Option<Taxonomy>> {
        let parsed_id = parse_uuid(id, "taxonomy_id")?;
        let mut conn = get_connection(&self.pool)?;
        let result = wf_taxonomies::table
            .find(parsed_id)
            .select(TaxonomyDB::as_select())
            .first::<TaxonomyDB>(&mut conn)
            .optional()
            .map_err(StorageError::from)?;
        Ok(result.map(Into::into))
    }

    async fn create_taxonomy(&self, taxonomy: NewTaxonomy) -> Result<Taxonomy> {
        let now = Utc::now();
        let db = TaxonomyDB {
            id: taxonomy
                .id
                .as_deref()
                .map(|value| parse_uuid(value, "taxonomy_id"))
                .transpose()?
                .unwrap_or_else(Uuid::new_v4),
            name: taxonomy.name,
            color: taxonomy.color,
            description: taxonomy.description,
            is_system: taxonomy.is_system,
            is_single_select: taxonomy.is_single_select,
            sort_order: taxonomy.sort_order,
            created_at: now,
            updated_at: now,
        };

        let mut conn = get_connection(&self.pool)?;
        let result = diesel::insert_into(wf_taxonomies::table)
            .values(&db)
            .get_result::<TaxonomyDB>(&mut conn)
            .map_err(StorageError::from)?;
        Ok(result.into())
    }

    async fn update_taxonomy(&self, taxonomy: Taxonomy) -> Result<Taxonomy> {
        let parsed_id = parse_uuid(&taxonomy.id, "taxonomy_id")?;
        let db = TaxonomyDB {
            id: parsed_id,
            name: taxonomy.name,
            color: taxonomy.color,
            description: taxonomy.description,
            is_system: taxonomy.is_system,
            is_single_select: taxonomy.is_single_select,
            sort_order: taxonomy.sort_order,
            created_at: DateTime::from_naive_utc_and_offset(taxonomy.created_at, Utc),
            updated_at: Utc::now(),
        };

        let mut conn = get_connection(&self.pool)?;
        let result = diesel::update(wf_taxonomies::table.find(parsed_id))
            .set(&db)
            .get_result::<TaxonomyDB>(&mut conn)
            .map_err(StorageError::from)?;
        Ok(result.into())
    }

    async fn delete_taxonomy(&self, id: &str) -> Result<usize> {
        let parsed_id = parse_uuid(id, "taxonomy_id")?;
        let mut conn = get_connection(&self.pool)?;
        diesel::delete(wf_taxonomies::table.find(parsed_id))
            .execute(&mut conn)
            .map_err(StorageError::from)
            .map_err(Into::into)
    }

    fn get_categories(&self, taxonomy_id: &str) -> Result<Vec<Category>> {
        let parsed_taxonomy_id = parse_uuid(taxonomy_id, "taxonomy_id")?;
        let mut conn = get_connection(&self.pool)?;
        let results = wf_taxonomy_categories::table
            .filter(wf_taxonomy_categories::taxonomy_id.eq(parsed_taxonomy_id))
            .order(wf_taxonomy_categories::sort_order.asc())
            .select(CategoryDB::as_select())
            .load::<CategoryDB>(&mut conn)
            .map_err(StorageError::from)?;
        Ok(results.into_iter().map(Into::into).collect())
    }

    fn get_category(&self, taxonomy_id: &str, category_id: &str) -> Result<Option<Category>> {
        let parsed_taxonomy_id = parse_uuid(taxonomy_id, "taxonomy_id")?;
        let parsed_category_id = parse_uuid(category_id, "category_id")?;
        let mut conn = get_connection(&self.pool)?;
        let result = wf_taxonomy_categories::table
            .filter(wf_taxonomy_categories::taxonomy_id.eq(parsed_taxonomy_id))
            .filter(wf_taxonomy_categories::id.eq(parsed_category_id))
            .select(CategoryDB::as_select())
            .first::<CategoryDB>(&mut conn)
            .optional()
            .map_err(StorageError::from)?;
        Ok(result.map(Into::into))
    }

    async fn create_category(&self, category: NewCategory) -> Result<Category> {
        let now = Utc::now();
        let db = CategoryDB {
            id: category
                .id
                .as_deref()
                .map(|value| parse_uuid(value, "category_id"))
                .transpose()?
                .unwrap_or_else(Uuid::new_v4),
            taxonomy_id: parse_uuid(&category.taxonomy_id, "taxonomy_id")?,
            parent_id: category
                .parent_id
                .as_deref()
                .map(|value| parse_uuid(value, "parent_id"))
                .transpose()?,
            name: category.name,
            key: category.key,
            color: category.color,
            description: category.description,
            sort_order: category.sort_order,
            created_at: now,
            updated_at: now,
        };

        let mut conn = get_connection(&self.pool)?;
        let result = diesel::insert_into(wf_taxonomy_categories::table)
            .values(&db)
            .get_result::<CategoryDB>(&mut conn)
            .map_err(StorageError::from)?;
        Ok(result.into())
    }

    async fn update_category(&self, category: Category) -> Result<Category> {
        let parsed_taxonomy_id = parse_uuid(&category.taxonomy_id, "taxonomy_id")?;
        let parsed_category_id = parse_uuid(&category.id, "category_id")?;
        let db = CategoryDB {
            id: parsed_category_id,
            taxonomy_id: parsed_taxonomy_id,
            parent_id: category
                .parent_id
                .as_deref()
                .map(|value| parse_uuid(value, "parent_id"))
                .transpose()?,
            name: category.name,
            key: category.key,
            color: category.color,
            description: category.description,
            sort_order: category.sort_order,
            created_at: DateTime::from_naive_utc_and_offset(category.created_at, Utc),
            updated_at: Utc::now(),
        };

        let mut conn = get_connection(&self.pool)?;
        let result = diesel::update(
            wf_taxonomy_categories::table
                .filter(wf_taxonomy_categories::taxonomy_id.eq(parsed_taxonomy_id))
                .filter(wf_taxonomy_categories::id.eq(parsed_category_id)),
        )
        .set(&db)
        .get_result::<CategoryDB>(&mut conn)
        .map_err(StorageError::from)?;
        Ok(result.into())
    }

    async fn delete_category(&self, taxonomy_id: &str, category_id: &str) -> Result<usize> {
        let parsed_taxonomy_id = parse_uuid(taxonomy_id, "taxonomy_id")?;
        let parsed_category_id = parse_uuid(category_id, "category_id")?;
        let mut conn = get_connection(&self.pool)?;
        diesel::delete(
            wf_taxonomy_categories::table
                .filter(wf_taxonomy_categories::taxonomy_id.eq(parsed_taxonomy_id))
                .filter(wf_taxonomy_categories::id.eq(parsed_category_id)),
        )
        .execute(&mut conn)
        .map_err(StorageError::from)
        .map_err(Into::into)
    }

    async fn bulk_create_categories(&self, categories: Vec<NewCategory>) -> Result<usize> {
        let mut inserted = 0;
        for category in categories {
            self.create_category(category).await?;
            inserted += 1;
        }
        Ok(inserted)
    }

    fn get_asset_assignments(&self, asset_id: &str) -> Result<Vec<AssetTaxonomyAssignment>> {
        let parsed_asset_id = parse_uuid(asset_id, "asset_id")?;
        let mut conn = get_connection(&self.pool)?;
        let results = wf_asset_taxonomy_assignments::table
            .filter(wf_asset_taxonomy_assignments::asset_id.eq(parsed_asset_id))
            .select(AssetTaxonomyAssignmentDB::as_select())
            .load::<AssetTaxonomyAssignmentDB>(&mut conn)
            .map_err(StorageError::from)?;
        Ok(results.into_iter().map(Into::into).collect())
    }

    fn get_category_assignments(
        &self,
        taxonomy_id: &str,
        category_id: &str,
    ) -> Result<Vec<AssetTaxonomyAssignment>> {
        let parsed_taxonomy_id = parse_uuid(taxonomy_id, "taxonomy_id")?;
        let parsed_category_id = parse_uuid(category_id, "category_id")?;
        let mut conn = get_connection(&self.pool)?;
        let results = wf_asset_taxonomy_assignments::table
            .filter(wf_asset_taxonomy_assignments::taxonomy_id.eq(parsed_taxonomy_id))
            .filter(wf_asset_taxonomy_assignments::category_id.eq(parsed_category_id))
            .select(AssetTaxonomyAssignmentDB::as_select())
            .load::<AssetTaxonomyAssignmentDB>(&mut conn)
            .map_err(StorageError::from)?;
        Ok(results.into_iter().map(Into::into).collect())
    }

    async fn upsert_assignment(
        &self,
        assignment: NewAssetTaxonomyAssignment,
    ) -> Result<AssetTaxonomyAssignment> {
        let now = Utc::now();
        let db = AssetTaxonomyAssignmentDB {
            id: assignment
                .id
                .as_deref()
                .map(|value| parse_uuid(value, "assignment_id"))
                .transpose()?
                .unwrap_or_else(Uuid::new_v4),
            asset_id: parse_uuid(&assignment.asset_id, "asset_id")?,
            taxonomy_id: parse_uuid(&assignment.taxonomy_id, "taxonomy_id")?,
            category_id: parse_uuid(&assignment.category_id, "category_id")?,
            weight: assignment.weight,
            source: assignment.source,
            created_at: now,
            updated_at: now,
        };

        let mut conn = get_connection(&self.pool)?;
        let result = diesel::insert_into(wf_asset_taxonomy_assignments::table)
            .values(&db)
            .on_conflict((
                wf_asset_taxonomy_assignments::asset_id,
                wf_asset_taxonomy_assignments::taxonomy_id,
                wf_asset_taxonomy_assignments::category_id,
            ))
            .do_update()
            .set((
                wf_asset_taxonomy_assignments::weight.eq(&db.weight),
                wf_asset_taxonomy_assignments::source.eq(&db.source),
                wf_asset_taxonomy_assignments::updated_at.eq(Utc::now()),
            ))
            .get_result::<AssetTaxonomyAssignmentDB>(&mut conn)
            .map_err(StorageError::from)?;
        Ok(result.into())
    }

    async fn delete_assignment(&self, id: &str) -> Result<usize> {
        let parsed_id = parse_uuid(id, "assignment_id")?;
        let mut conn = get_connection(&self.pool)?;
        diesel::delete(wf_asset_taxonomy_assignments::table.find(parsed_id))
            .execute(&mut conn)
            .map_err(StorageError::from)
            .map_err(Into::into)
    }

    async fn delete_asset_assignments(&self, asset_id: &str, taxonomy_id: &str) -> Result<usize> {
        let parsed_asset_id = parse_uuid(asset_id, "asset_id")?;
        let parsed_taxonomy_id = parse_uuid(taxonomy_id, "taxonomy_id")?;
        let mut conn = get_connection(&self.pool)?;
        diesel::delete(
            wf_asset_taxonomy_assignments::table
                .filter(wf_asset_taxonomy_assignments::asset_id.eq(parsed_asset_id))
                .filter(wf_asset_taxonomy_assignments::taxonomy_id.eq(parsed_taxonomy_id)),
        )
        .execute(&mut conn)
        .map_err(StorageError::from)
        .map_err(Into::into)
    }

    fn get_taxonomy_with_categories(&self, id: &str) -> Result<Option<TaxonomyWithCategories>> {
        match self.get_taxonomy(id)? {
            Some(taxonomy) => Ok(Some(TaxonomyWithCategories {
                categories: self.get_categories(id)?,
                taxonomy,
            })),
            None => Ok(None),
        }
    }

    fn get_all_taxonomies_with_categories(&self) -> Result<Vec<TaxonomyWithCategories>> {
        let taxonomies = self.get_taxonomies()?;
        let mut result = Vec::with_capacity(taxonomies.len());
        for taxonomy in taxonomies {
            let categories = self.get_categories(&taxonomy.id)?;
            result.push(TaxonomyWithCategories {
                taxonomy,
                categories,
            });
        }
        Ok(result)
    }
}