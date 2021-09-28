use crate::{ApubObject, Crud};
use chrono::NaiveDateTime;
use diesel::{dsl::*, result::Error, *};
use lemmy_db_schema::{naive_now, source::site::*, DbUrl, PersonId};

impl Crud for Site {
  type Form = SiteForm;
  type IdType = i32;
  fn read(_conn: &PgConnection, _site_id: i32) -> Result<Self, Error> {
    unimplemented!()
  }

  fn create(conn: &PgConnection, new_site: &SiteForm) -> Result<Self, Error> {
    use lemmy_db_schema::schema::site::dsl::*;
    insert_into(site).values(new_site).get_result::<Self>(conn)
  }

  fn update(conn: &PgConnection, site_id: i32, new_site: &SiteForm) -> Result<Self, Error> {
    use lemmy_db_schema::schema::site::dsl::*;
    diesel::update(site.find(site_id))
      .set(new_site)
      .get_result::<Self>(conn)
  }
  fn delete(conn: &PgConnection, site_id: i32) -> Result<usize, Error> {
    use lemmy_db_schema::schema::site::dsl::*;
    diesel::delete(site.find(site_id)).execute(conn)
  }
}

pub trait Site_ {
  fn transfer(conn: &PgConnection, new_creator_id: PersonId) -> Result<Site, Error>;
  fn read_local_site(conn: &PgConnection) -> Result<Site, Error>;
  fn upsert(conn: &PgConnection, site_form: &SiteForm) -> Result<Site, Error>;
}

impl Site_ for Site {
  fn transfer(conn: &PgConnection, new_creator_id: PersonId) -> Result<Site, Error> {
    use lemmy_db_schema::schema::site::dsl::*;
    diesel::update(site.find(1))
      .set((creator_id.eq(new_creator_id), updated.eq(naive_now())))
      .get_result::<Self>(conn)
  }

  fn read_local_site(conn: &PgConnection) -> Result<Self, Error> {
    use lemmy_db_schema::schema::site::dsl::*;
    site.order_by(id).first::<Self>(conn)
  }

  fn upsert(conn: &PgConnection, site_form: &SiteForm) -> Result<Site, Error> {
    use lemmy_db_schema::schema::site::dsl::*;
    insert_into(site)
      .values(site_form)
      .on_conflict(actor_id)
      .do_update()
      .set(site_form)
      .get_result::<Self>(conn)
  }
}

impl ApubObject for Site {
  fn last_refreshed_at(&self) -> Option<NaiveDateTime> {
    Some(self.last_refreshed_at)
  }

  fn read_from_apub_id(conn: &PgConnection, object_id: &DbUrl) -> Result<Self, Error> {
    use lemmy_db_schema::schema::site::dsl::*;
    site.filter(actor_id.eq(object_id)).first::<Self>(conn)
  }
}
