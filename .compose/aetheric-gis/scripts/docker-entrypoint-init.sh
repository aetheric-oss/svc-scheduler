## DO NOT EDIT!
# This file was provisioned by OpenTofu
# File origin: https://github.com/aetheric-oss/tofu-github/tree/main/src/modules/vars/templates/rust/svc/.compose/aetheric-gis/scripts/docker-entrypoint-init.sh

#!/bin/bash

if [[ -f "$PGDATA/init_done" ]]
then
	echo Init not needed, done.
	exit 0
fi

# we -need- trust to init our database
export POSTGRES_HOST_AUTH_METHOD=trust

# make sure our data dir is empty (in case we failed half way through previously)
rm -rf -- /var/lib/postgresql/data/*

# Start docker entry point to trigger init steps
/usr/local/bin/docker-entrypoint.sh postgres &
ENTRYPOINT_PID=$!

# Wait till init scripts completed
# We can test this by calling one of the functions being created by our init script.
while ! psql -U svc_gis -d $POSTGRES_DB -c "SELECT EXISTS ( SELECT 1 FROM information_schema.schemata WHERE schema_name = 'aetheric');"
do
	echo Waiting for init to be done...
	sleep 2
done

kill -0 $ENTRYPOINT_PID
DB_RUNNING=$?
echo The database entrypoint script has stopped? $DB_RUNNING

if [[ $DB_RUNNING == 0 ]]
then
	# Enable SSL
	psql -U postgres -c "ALTER SYSTEM SET ssl = 'on';"
	psql -U postgres -c "ALTER SYSTEM SET ssl_cert_file = '$PGSSLCERT';"
	psql -U postgres -c "ALTER SYSTEM SET ssl_key_file = '$PGSSLKEY';"
	psql -U postgres -c "ALTER SYSTEM SET ssl_ca_file = '$PGSSLROOTCERT';"

	# Overwrite pg_hba.conf so we only accept SSL cert connections.
	printf "%s\n# Only accept SSL connections with certificate and local tools.\n%s\n%s\n" \
		"# TYPE  DATABASE        USER            ADDRESS                 METHOD" \
		"local   all             all                                     trust"\
		"hostssl all             all             all                     cert" > /var/lib/postgresql/data/pg_hba.conf

	echo Content of pg_hba.conf is now set to require ssl for non-local users
	cat /var/lib/postgresql/data/pg_hba.conf

	touch "$PGDATA/init_done"
	echo Init done.
	exit 0
else
	echo Failed to init
	exit 1
fi
