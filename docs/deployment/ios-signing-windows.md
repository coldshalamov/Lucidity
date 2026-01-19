# iOS Signing Without a Mac

You can build and release iOS apps using GitHub Actions (which provides cloud Macs), but you need to generate the signing certificates first. This guide explains how to do that on Windows.

## Prerequisites

1.  **Apple Developer Program Membership** ($99/year) - Required to release to App Store/TestFlight.
2.  **OpenSSL** - Installed via Git Bash (included with Git for Windows) or WSL.

## Step 1: Generate a Certificate Signing Request (CSR)

Run the following commands in your terminal (Git Bash or PowerShell with OpenSSL):

```bash
# 1. Generate a generic private key
openssl genrsa -out ios_distribution.key 2048

# 2. Generate the CSR (Certificate Signing Request)
# Email: Use your Apple ID email
# CN: Your name or company name (as it appears in Apple Developer Portal)
openssl req -new -key ios_distribution.key -out ios_distribution.csr -subj "/emailAddress=you@example.com, CN=Your Name, C=US"
```

## Step 2: Create the Certificate

1.  Log in to [Apple Developer Portal](https://developer.apple.com/account).
2.  Go to **Certificates, Identifiers & Profiles** > **Certificates**.
3.  Click `+`.
4.  Select **Apple Distribution** (or iOS Distribution).
5.  Upload the `ios_distribution.csr` file you created in Step 1.
6.  Download the generated `.cer` file (e.g., `ios_distribution.cer`).

## Step 3: Convert to P12 Format

GitHub Actions needs a `.p12` file (which contains both the certificate and the private key).

```bash
# 1. Convert the downloaded .cer to .pem
openssl x509 -in ios_distribution.cer -inform DER -out ios_distribution.pem -outform PEM

# 2. Export p12
# You will be asked to set a password. REMEMBER THIS PASSWORD.
# You will need to add it to GitHub Secrets as IOS_P12_PASSWORD.
openssl pkcs12 -export -inkey ios_distribution.key -in ios_distribution.pem -out build_certificate.p12
```

## Step 4: Convert to Base64 for GitHub Secrets

GitHub Secrets cannot store binary files directly, so we convert them to Base64 text.

```bash
# Windows PowerShell
[Convert]::ToBase64String([IO.File]::ReadAllBytes("build_certificate.p12")) | Set-Clipboard
```

Paste this value into GitHub Secret `IOS_BUILD_CERTIFICATE_BASE64`.

## Step 5: Provisioning Profile

1.  In Apple Developer Portal, go to **Identifiers**.
2.  Create an App ID (e.g., `com.yourname.lucidity`).
3.  Go to **Profiles**.
4.  Create a new **App Store** distribution profile.
5.  Select the App ID and the Certificate you created.
6.  Download the `.mobileprovision` file.
7.  Convert to Base64:

```bash
# Windows PowerShell
[Convert]::ToBase64String([IO.File]::ReadAllBytes("Lucidity_Dist.mobileprovision")) | Set-Clipboard
```

Paste this into GitHub Secret `IOS_PROVISION_PROFILE_BASE64`.

## Summary of Secrets Needed

| Secret Name | Value |
|-------------|-------|
| `IOS_BUILD_CERTIFICATE_BASE64` | Content of `build_certificate.p12` (Base64) |
| `IOS_P12_PASSWORD` | Password you set in Step 3 |
| `IOS_PROVISION_PROFILE_BASE64` | Content of `.mobileprovision` (Base64) |
| `IOS_KEYCHAIN_PASSWORD` | Any random string (used temporarily by the build runner) |
