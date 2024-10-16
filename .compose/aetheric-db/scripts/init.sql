-- DO NOT EDIT!
-- This file was provisioned by OpenTofu
-- File origin: https://github.com/aetheric-oss/tofu-github/tree/main/src/modules/vars/templates/rust/svc/.compose/aetheric-db/scripts/init.sql

CREATE DATABASE realm;
CREATE USER svc_storage;
GRANT ALL PRIVILEGES ON DATABASE realm TO svc_storage;

-- Create db user for dashboard login (Only for dev!)
CREATE USER developer WITH LOGIN PASSWORD 'dev_login';
GRANT SYSTEM VIEWACTIVITY TO developer;
GRANT SYSTEM VIEWCLUSTERMETADATA TO developer;
GRANT SYSTEM VIEWCLUSTERSETTING TO developer;
GRANT SYSTEM VIEWDEBUG TO developer;
GRANT SYSTEM VIEWJOB TO developer;
GRANT SYSTEM VIEWSYSTEMTABLE TO developer;
