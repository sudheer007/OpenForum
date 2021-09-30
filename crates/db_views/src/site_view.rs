use diesel::{result::Error, *};
use lemmy_db_queries::aggregates::site_aggregates::SiteAggregates;
use lemmy_db_schema::{
  schema::{site, site_aggregates},
  source::site::Site,
};
use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub struct SiteView {
  pub site: Site,
  pub counts: SiteAggregates,
}

impl SiteView {
  pub fn read(conn: &PgConnection) -> Result<Self, Error> {
    let (site, counts) = site::table
      .inner_join(site_aggregates::table)
      .select((site::all_columns, site_aggregates::all_columns))
      .first::<(Site, SiteAggregates)>(conn)?;

    Ok(SiteView { site, counts })
  }
}
