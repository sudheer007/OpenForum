use crate::{
  check_is_apub_id_valid,
  extensions::{context::lemmy_context, signatures::PublicKey},
  fetcher::{
    community::{fetch_community_outbox, update_community_mods},
    object_id::ObjectId,
  },
  generate_moderators_url,
  objects::{
    create_tombstone,
    instance::instance_actor_id_from_url,
    FromApub,
    ImageObject,
    Source,
    ToApub,
  },
  ActorType,
  CommunityType,
};
use activitystreams::{
  actor::{kind::GroupType, Endpoints},
  base::AnyBase,
  object::Tombstone,
  primitives::OneOrMany,
  unparsed::Unparsed,
};
use chrono::{DateTime, FixedOffset};
use itertools::Itertools;
use lemmy_api_common::blocking;
use lemmy_apub_lib::{
  values::{MediaTypeHtml, MediaTypeMarkdown},
  verify_domains_match,
};
use lemmy_db_queries::{source::community::Community_, DbPool};
use lemmy_db_schema::{
  naive_now,
  source::{
    community::{Community, CommunityForm},
    site::Site,
  },
};
use lemmy_db_views_actor::community_follower_view::CommunityFollowerView;
use lemmy_utils::{
  settings::structs::Settings,
  utils::{check_slurs, check_slurs_opt, convert_datetime, markdown_to_html},
  LemmyError,
};
use lemmy_websocket::LemmyContext;
use log::warn;
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;
use url::Url;

#[skip_serializing_none]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
  #[serde(rename = "@context")]
  context: OneOrMany<AnyBase>,
  #[serde(rename = "type")]
  kind: GroupType,
  id: Url,
  /// username, set at account creation and can never be changed
  preferred_username: String,
  /// title (can be changed at any time)
  name: String,
  content: Option<String>,
  media_type: Option<MediaTypeHtml>,
  source: Option<Source>,
  icon: Option<ImageObject>,
  /// banner
  image: Option<ImageObject>,
  // lemmy extension
  sensitive: Option<bool>,
  // lemmy extension
  pub(crate) moderators: Option<Url>,
  inbox: Url,
  pub(crate) outbox: Url,
  followers: Url,
  endpoints: Endpoints<Url>,
  public_key: PublicKey,
  published: DateTime<FixedOffset>,
  updated: Option<DateTime<FixedOffset>>,
  #[serde(flatten)]
  unparsed: Unparsed,
}

impl ActorType for Community {
  fn is_local(&self) -> bool {
    self.local
  }
  fn actor_id(&self) -> Url {
    self.actor_id.to_owned().into()
  }
  fn name(&self) -> String {
    self.name.clone()
  }
  fn public_key(&self) -> Option<String> {
    self.public_key.to_owned()
  }
  fn private_key(&self) -> Option<String> {
    self.private_key.to_owned()
  }

  fn get_shared_inbox_or_inbox_url(&self) -> Url {
    self
      .shared_inbox_url
      .clone()
      .unwrap_or_else(|| self.inbox_url.to_owned())
      .into()
  }
}

#[async_trait::async_trait(?Send)]
impl CommunityType for Community {
  fn followers_url(&self) -> Url {
    self.followers_url.clone().into()
  }

  /// For a given community, returns the inboxes of all followers.
  async fn get_follower_inboxes(
    &self,
    pool: &DbPool,
    settings: &Settings,
  ) -> Result<Vec<Url>, LemmyError> {
    let id = self.id;

    let follows = blocking(pool, move |conn| {
      CommunityFollowerView::for_community(conn, id)
    })
    .await??;
    let inboxes = follows
      .into_iter()
      .filter(|f| !f.follower.local)
      .map(|f| f.follower.shared_inbox_url.unwrap_or(f.follower.inbox_url))
      .map(|i| i.into_inner())
      .unique()
      // Don't send to blocked instances
      .filter(|inbox| check_is_apub_id_valid(inbox, false, settings).is_ok())
      .collect();

    Ok(inboxes)
  }
}

impl Group {
  pub(crate) fn id(&self, expected_domain: &Url) -> Result<&Url, LemmyError> {
    verify_domains_match(&self.id, expected_domain)?;
    Ok(&self.id)
  }
  pub(crate) async fn from_apub_to_form(
    group: &Group,
    expected_domain: &Url,
    settings: &Settings,
  ) -> Result<CommunityForm, LemmyError> {
    let actor_id = Some(group.id(expected_domain)?.clone().into());
    let name = group.preferred_username.clone();
    let title = group.name.clone();
    let description = group.source.clone().map(|s| s.content);
    let shared_inbox = group.endpoints.shared_inbox.clone().map(|s| s.into());

    let slur_regex = &settings.slur_regex();
    check_slurs(&name, slur_regex)?;
    check_slurs(&title, slur_regex)?;
    check_slurs_opt(&description, slur_regex)?;

    Ok(CommunityForm {
      name,
      title,
      description,
      removed: None,
      published: Some(group.published.naive_local()),
      updated: group.updated.map(|u| u.naive_local()),
      deleted: None,
      nsfw: Some(group.sensitive.unwrap_or(false)),
      actor_id,
      local: Some(false),
      private_key: None,
      public_key: Some(group.public_key.public_key_pem.clone()),
      last_refreshed_at: Some(naive_now()),
      icon: Some(group.icon.clone().map(|i| i.url.into())),
      banner: Some(group.image.clone().map(|i| i.url.into())),
      followers_url: Some(group.followers.clone().into()),
      inbox_url: Some(group.inbox.clone().into()),
      shared_inbox_url: Some(shared_inbox),
    })
  }
}

#[async_trait::async_trait(?Send)]
impl ToApub for Community {
  type ApubType = Group;

  async fn to_apub(&self, _pool: &DbPool) -> Result<Group, LemmyError> {
    let source = self.description.clone().map(|bio| Source {
      content: bio,
      media_type: MediaTypeMarkdown::Markdown,
    });

    let group = Group {
      context: lemmy_context(),
      kind: GroupType::Group,
      id: self.actor_id(),
      preferred_username: self.name.clone(),
      name: self.title.clone(),
      content: self.description.as_ref().map(|b| markdown_to_html(b)),
      media_type: self.description.as_ref().map(|_| MediaTypeHtml::Html),
      source,
      icon: self.icon.clone().map(ImageObject::new),
      image: self.banner.clone().map(ImageObject::new),
      sensitive: Some(self.nsfw),
      moderators: Some(generate_moderators_url(&self.actor_id)?.into()),
      inbox: self.inbox_url.clone().into(),
      outbox: self.get_outbox_url()?,
      followers: self.followers_url.clone().into(),
      endpoints: Endpoints {
        shared_inbox: self.shared_inbox_url.clone().map(|s| s.into()),
        ..Default::default()
      },
      public_key: self.get_public_key()?,
      published: convert_datetime(self.published),
      updated: self.updated.map(convert_datetime),
      unparsed: Default::default(),
    };
    Ok(group)
  }

  fn to_tombstone(&self) -> Result<Tombstone, LemmyError> {
    create_tombstone(
      self.deleted,
      self.actor_id.to_owned().into(),
      self.updated,
      GroupType::Group,
    )
  }
}

#[async_trait::async_trait(?Send)]
impl FromApub for Community {
  type ApubType = Group;

  /// Converts a `Group` to `Community`, inserts it into the database and updates moderators.
  async fn from_apub(
    group: &Group,
    context: &LemmyContext,
    expected_domain: &Url,
    request_counter: &mut i32,
  ) -> Result<Community, LemmyError> {
    let form = Group::from_apub_to_form(group, expected_domain, &context.settings()).await?;

    let community = blocking(context.pool(), move |conn| Community::upsert(conn, &form)).await??;
    update_community_mods(group, &community, context, request_counter).await?;

    // TODO: doing this unconditionally might cause infinite loop for some reason
    fetch_community_outbox(context, &group.outbox, request_counter).await?;

    // try to fetch the instance actor (to make things like instance rules available)
    let instance_id = instance_actor_id_from_url(community.actor_id.clone().into());
    let site = ObjectId::<Site>::new(instance_id.clone())
      .dereference(context, request_counter)
      .await;
    if let Err(e) = site {
      warn!("Failed to dereference site for {}: {}", instance_id, e);
    }

    Ok(community)
  }
}
