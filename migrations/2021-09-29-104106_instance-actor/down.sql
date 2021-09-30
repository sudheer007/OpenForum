-- drop actor columns
alter table site
    drop column actor_id,
    drop column last_refreshed_at,
    drop column inbox_url,
    drop column private_key,
    drop column public_key;

--  Add the column back
alter table site add column creator_id int references person on update cascade on delete cascade;

-- Recreate the index
create index idx_site_creator on site (creator_id);

-- Add the data, selecting the highest mod
update site
set creator_id = admin.id
from (
  select
  person.id
  from
  person
  where admin = true
  limit 1
) as admin;

-- Set to not null
alter table site alter column creator_id set not null;