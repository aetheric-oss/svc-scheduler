-- DO NOT EDIT!
-- This file was provisioned by OpenTofu
-- File origin: https://github.com/aetheric-oss/tofu-github/tree/main/src/modules/vars/templates/rust/svc/.compose/aetheric-gis/scripts/init.sql

CREATE USER svc_gis;
CREATE EXTENSION postgis_sfcgal CASCADE;
\c gis
CREATE SCHEMA IF NOT EXISTS arrow;
GRANT ALL PRIVILEGES ON SCHEMA arrow TO svc_gis;
GRANT ALL ON SCHEMA public TO svc_gis;
ALTER DATABASE gis OWNER TO svc_gis;
