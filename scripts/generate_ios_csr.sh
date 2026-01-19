#!/bin/bash
set -e

# Configuration
KEY_NAME="ios_distribution"
COMMON_NAME="Lucidity Developer"
EMAIL="dev@lucidity.app"
OPENSSL_CMD="openssl"

echo "🍎 iOS Signing Helper for Windows/Linux 🍎"
echo "=========================================="

# Check openssl
if ! command -v $OPENSSL_CMD &> /dev/null; then
    echo "Error: openssl not found."
    echo "On Windows, use Git Bash."
    exit 1
fi

echo "1. Generating Private Key ($KEY_NAME.key)..."
$OPENSSL_CMD genrsa -out "$KEY_NAME.key" 2048

echo "2. Generating CSR ($KEY_NAME.csr)..."
$OPENSSL_CMD req -new -key "$KEY_NAME.key" -out "$KEY_NAME.csr" \
    -subj "//emailAddress=$EMAIL, CN=$COMMON_NAME, C=US"

echo ""
echo "✅ CSR Generated: $KEY_NAME.csr"
echo ""
echo "NEXT STEPS:"
echo "1. Upload '$KEY_NAME.csr' to https://developer.apple.com/account/resources/certificates/add"
echo "2. Download the resulting .cer file (e.g., ios_distribution.cer)"
echo "3. Run the following command to create the .p12 file:"
echo ""
echo "   openssl x509 -in ios_distribution.cer -inform DER -out ios_distribution.pem -outform PEM"
echo "   openssl pkcs12 -export -inkey $KEY_NAME.key -in ios_distribution.pem -out build_certificate.p12"
echo ""
