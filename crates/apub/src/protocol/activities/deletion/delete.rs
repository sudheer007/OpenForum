use crate::objects::person::ApubPerson;
use activitystreams::{
  activity::kind::DeleteType,
  object::kind::TombstoneType,
  unparsed::Unparsed,
};
use lemmy_apub_lib::object_id::ObjectId;
use serde::{Deserialize, Serialize};
use serde_with::skip_serializing_none;
use url::Url;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
#[serde(rename_all = "camelCase")]
pub enum ObjectOrTombstone {
  Url(Url),
  Tombstone {
    id: Url,
    #[serde(rename = "type")]
    kind: TombstoneType,
  },
}

#[skip_serializing_none]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Delete {
  pub(crate) actor: ObjectId<ApubPerson>,
  pub(crate) to: Vec<Url>,
  pub(crate) object: ObjectOrTombstone,
  pub(crate) cc: Option<Vec<Url>>,
  #[serde(rename = "type")]
  pub(crate) kind: DeleteType,
  /// If summary is present, this is a mod action (Remove in Lemmy terms). Otherwise, its a user
  /// deleting their own content.
  pub(crate) summary: Option<String>,
  pub(crate) id: Url,
  #[serde(flatten)]
  pub(crate) unparsed: Unparsed,
}

impl ObjectOrTombstone {
  pub fn as_url(&self) -> &Url {
    match self {
      Self::Url(ref url) => url,
      Self::Tombstone { ref id, .. } => id,
    }
  }
}
