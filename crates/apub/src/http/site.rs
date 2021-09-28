use crate::{http::create_apub_response, objects::ToApub};
use actix_web::{body::Body, web, HttpResponse};
use lemmy_api_common::blocking;
use lemmy_db_queries::source::site::Site_;
use lemmy_db_schema::source::site::Site;
use lemmy_utils::LemmyError;
use lemmy_websocket::LemmyContext;

pub(crate) async fn get_apub_site_http(
  context: web::Data<LemmyContext>,
) -> Result<HttpResponse<Body>, LemmyError> {
  let site = blocking(context.pool(), move |conn| Site::read_local_site(conn)).await??;

  let apub = site.to_apub(context.pool()).await?;
  Ok(create_apub_response(&apub))
}
