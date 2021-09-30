use crate::{
  extensions::{context::lemmy_context, signatures::PublicKey},
  objects::{FromApub, ImageObject, ToApub},
  ActorType,
};
use activitystreams::{
  actor::kind::ServiceType,
  base::AnyBase,
  chrono::{DateTime, FixedOffset},
  object::Tombstone,
  primitives::OneOrMany,
  unparsed::Unparsed,
};
use lemmy_api_common::blocking;
use lemmy_apub_lib::{values::MediaTypeMarkdown, verify_domains_match};
use lemmy_db_queries::{source::site::Site_, DbPool};
use lemmy_db_schema::{
  naive_now,
  source::site::{Site, SiteForm},
};
use lemmy_utils::{settings::structs::Settings, utils::convert_datetime, LemmyError};
use lemmy_websocket::LemmyContext;
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;
use url::Url;

#[skip_serializing_none]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Instance {
  #[serde(rename = "@context")]
  context: OneOrMany<AnyBase>,
  #[serde(rename = "type")]
  kind: ServiceType,
  id: Url,
  name: String,
  content: Option<String>,
  summary: Option<String>,
  media_type: Option<MediaTypeMarkdown>,
  /// instance icon
  icon: Option<ImageObject>,
  /// instance banner
  image: Option<ImageObject>,
  inbox: Url,
  /// mandatory field in activitypub, currently empty in lemmy
  outbox: Url,
  public_key: PublicKey,
  published: DateTime<FixedOffset>,
  updated: Option<DateTime<FixedOffset>>,
  #[serde(flatten)]
  unparsed: Unparsed,
}

impl ActorType for Site {
  fn is_local(&self) -> bool {
    todo!()
  }

  fn actor_id(&self) -> Url {
    self.actor_id.clone().into()
  }

  fn name(&self) -> String {
    self.name.clone()
  }

  fn public_key(&self) -> Option<String> {
    Some(self.public_key.clone())
  }

  fn private_key(&self) -> Option<String> {
    self.private_key.clone()
  }

  fn get_shared_inbox_or_inbox_url(&self) -> Url {
    self.inbox_url.clone().into()
  }
}

#[async_trait::async_trait(?Send)]
impl ToApub for Site {
  type ApubType = Instance;

  async fn to_apub(&self, _pool: &DbPool) -> Result<Instance, LemmyError> {
    let instance = Instance {
      context: lemmy_context(),
      kind: ServiceType::Service,
      id: Url::parse(&Settings::get().get_protocol_and_hostname())?,
      name: self.name.clone(),
      content: self.sidebar.clone(),
      summary: self.description.clone(),
      media_type: self.sidebar.as_ref().map(|_| MediaTypeMarkdown::Markdown),
      icon: self.icon.clone().map(ImageObject::new),
      image: self.banner.clone().map(ImageObject::new),
      inbox: self.inbox_url.clone().into(),
      outbox: self.get_outbox_url()?,
      public_key: self.get_public_key()?,
      published: convert_datetime(self.published),
      updated: self.updated.map(convert_datetime),
      unparsed: Default::default(),
    };
    Ok(instance)
  }
  fn to_tombstone(&self) -> Result<Tombstone, LemmyError> {
    unimplemented!()
  }
}

#[async_trait::async_trait(?Send)]
impl FromApub for Site {
  type ApubType = Instance;

  async fn from_apub(
    instance: &Instance,
    context: &LemmyContext,
    expected_domain: &Url,
    _request_counter: &mut i32,
  ) -> Result<Site, LemmyError> {
    verify_domains_match(&instance.id, expected_domain)?;
    let site_form = SiteForm {
      name: instance.name.clone(),
      sidebar: Some(instance.content.clone()),
      updated: instance.updated.map(|u| u.clone().naive_local()),
      enable_downvotes: None,
      open_registration: None,
      enable_nsfw: None,
      icon: Some(instance.icon.clone().map(|i| i.url.into())),
      banner: Some(instance.image.clone().map(|i| i.url.into())),
      description: Some(instance.summary.clone()),
      community_creation_admin_only: None,
      actor_id: Some(instance.id.clone().into()),
      last_refreshed_at: Some(naive_now()),
      inbox_url: Some(instance.inbox.clone().into()),
      private_key: None,
      public_key: Some(instance.public_key.public_key_pem.clone()),
    };
    let site = blocking(context.pool(), move |conn| Site::upsert(conn, &site_form)).await??;
    Ok(site)
  }
}

/// Instance actor is at the root path, so we simply need to clear the path and other unnecessary
/// parts of the url.
pub(crate) fn instance_actor_id_from_url(mut url: Url) -> Url {
  url.set_fragment(None);
  url.set_path("");
  url.set_query(None);
  url
}
