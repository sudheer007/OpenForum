use crate::PerformCrud;
use actix_web::web::Data;
use lemmy_api_common::{
  blocking,
  get_local_user_view_from_jwt,
  is_admin,
  site::*,
  site_description_length_check,
};
use lemmy_apub::generate_inbox_url;
use lemmy_db_queries::{
  diesel_option_overwrite,
  diesel_option_overwrite_to_url,
  source::site::Site_,
  Crud,
};
use lemmy_db_schema::{
  naive_now,
  source::site::{Site, *},
  DbUrl,
};
use lemmy_db_views::site_view::SiteView;
use lemmy_utils::{
  apub::generate_actor_keypair,
  settings::structs::Settings,
  utils::{check_slurs, check_slurs_opt},
  ApiError,
  ConnectionId,
  LemmyError,
};
use lemmy_websocket::LemmyContext;
use url::Url;

#[async_trait::async_trait(?Send)]
impl PerformCrud for CreateSite {
  type Response = SiteResponse;

  async fn perform(
    &self,
    context: &Data<LemmyContext>,
    _websocket_id: Option<ConnectionId>,
  ) -> Result<SiteResponse, LemmyError> {
    let data: &CreateSite = self;

    let read_site = move |conn: &'_ _| Site::read_local_site(conn);
    if blocking(context.pool(), read_site).await?.is_ok() {
      return Err(ApiError::err("site_already_exists").into());
    };

    let local_user_view =
      get_local_user_view_from_jwt(&data.auth, context.pool(), context.secret()).await?;

    check_slurs(&data.name, &context.settings().slur_regex())?;
    check_slurs_opt(&data.description, &context.settings().slur_regex())?;

    // Make sure user is an admin
    is_admin(&local_user_view)?;

    let sidebar = diesel_option_overwrite(&data.sidebar);
    let description = diesel_option_overwrite(&data.description);
    let icon = diesel_option_overwrite_to_url(&data.icon)?;
    let banner = diesel_option_overwrite_to_url(&data.banner)?;

    if let Some(Some(desc)) = &description {
      site_description_length_check(desc)?;
    }

    let actor_id: DbUrl = Url::parse(&Settings::get().get_protocol_and_hostname())?.into();
    let keypair = generate_actor_keypair()?;
    let site_form = SiteForm {
      name: data.name.to_owned(),
      sidebar,
      description,
      icon,
      banner,
      enable_downvotes: data.enable_downvotes,
      open_registration: data.open_registration,
      enable_nsfw: data.enable_nsfw,
      updated: None,
      community_creation_admin_only: data.community_creation_admin_only,
      actor_id: Some(actor_id.clone()),
      last_refreshed_at: Some(naive_now()),
      inbox_url: Some(generate_inbox_url(&actor_id)?),
      private_key: Some(Some(keypair.private_key)),
      public_key: Some(keypair.public_key),
    };

    let create_site = move |conn: &'_ _| Site::create(conn, &site_form);
    if blocking(context.pool(), create_site).await?.is_err() {
      return Err(ApiError::err("site_already_exists").into());
    }

    let site_view = blocking(context.pool(), move |conn| SiteView::read(conn)).await??;

    Ok(SiteResponse { site_view })
  }
}
