## DO NOT EDIT!
# This file was provisioned by OpenTofu
# File origin: https://github.com/aetheric-oss/tofu-github/tree/main/src/modules/vars/templates/rust/svc/.compose/aetheric-gis/scripts/init-ssl.sh

#!/bin/sh

SSL_DIR=/ssl
CERT_DIR=${SSL_DIR}/certs
KEY_DIR=${SSL_DIR}/keys
CLIENT=svc_gis # name of the user connecting to this server
SERVER_HOSTNAME=aetheric-gis # hostname used for the connection to this server

set -e

if [ ! -f "/.dockerenv" ]; then
	printf "%s\n" "This script is meant as init-script for postgis in docker. Refusing to run here."
	exit 0
fi

# Do we have expected volume/mounts? Create dir if nothing's there.
for d in "${SSL_DIR}" "${CERT_DIR}" "${KEY_DIR}"
do
	if [ ! -d "${d}" ]; then
		if [ ! -e "${d}" ]; then
			printf "%s\n"  "Creating non-existing dir ${d}"
			mkdir -p "${d}"
		else
			printf "%s\n" "SSL related directory expected at ${d} is not a dir. Refusing."
			exit 1
		fi
	fi
done

# Debug
printf "Using SSL dirs:\nSSL base:\t%s\nCert dir:\t%s\nKey dir:\t%s\n\n" "${SSL_DIR}" "${CERT_DIR}" "${KEY_DIR}"

# Root cert
if [ ! -f "${CERT_DIR}/root.crt" ]; then
	printf "%s\n" "Creating root certificate request...."

	# Create Root CA Request
	openssl req -new -nodes -text \
		-subj "/CN=${SERVER_HOSTNAME}" \
		-keyout ${KEY_DIR}/root.key \
		-out ${CERT_DIR}/root.csr

	printf "%s\n" "Signing root certificate request...."

	# Sign the Request to Make a Root CA certificate
	openssl x509 -req -text -days 3650 \
		-signkey ${KEY_DIR}/root.key \
		-in ${CERT_DIR}/root.csr \
		-out ${CERT_DIR}/root.crt
fi

# Client cert
if [ ! -f "${CERT_DIR}/client.${CLIENT}.crt" ]; then
	printf "%s\n" "Creating ${CLIENT} certificate request...."

	# Create Client CA Request
	openssl req -new -nodes -text \
		-subj "/CN=${CLIENT}" \
		-keyout ${KEY_DIR}/client.${CLIENT}.key \
		-out ${CERT_DIR}/client.${CLIENT}.csr

	printf "%s\n" "Signing client request with root CA...."

	# Use the Root CA to Sign the Client Certificate
	openssl x509 -req -text -days 3650 -CAcreateserial \
		-CA ${CERT_DIR}/root.crt \
		-CAkey ${KEY_DIR}/root.key \
		-in ${CERT_DIR}/client.${CLIENT}.csr \
		-out ${CERT_DIR}/client.${CLIENT}.crt

	# Create PKCS#8 format key
	openssl pkcs8 -topk8 -outform PEM -nocrypt \
		-in ${KEY_DIR}/client.${CLIENT}.key \
		-out ${KEY_DIR}/client.${CLIENT}.key.pk8
fi

echo Making sure the postgres user can read the certs to start the server
chown -R postgres:postgres ${SSL_DIR}/
# make sure pk8 file is readable by docker user
chown $DOCKER_USER_ID:$DOCKER_GROUP_ID "${CERT_DIR}/client.${CLIENT}.key.pk8"

exit 0
