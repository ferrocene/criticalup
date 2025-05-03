// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::keys::KeyRole;
use std::string::FromUtf8Error;
use thiserror::Error;

#[non_exhaustive]
#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    FromUtf8(#[from] FromUtf8Error),
    #[error("Failed to sign data.")]
    SignatureFailed,
    #[error("Failed to verify signed data.")]
    VerificationFailed,
    #[error("Failed to generate a local key.")]
    LocalKeyGenerationFailed,
    #[error("Wrong key role for the trust root key (expected Root, found {0:?}).")]
    WrongKeyRoleForTrustRoot(KeyRole),
    #[error("Failed to serialize the contents of the signed payload.")]
    SignedPayloadSerializationFailed(#[source] serde_json::Error),
    #[error("Failed to deserialize verified data.")]
    DeserializationFailed(#[source] serde_json::Error),
    #[error("Failed to load key pair.")]
    InvalidKey(String),
    #[error("Unsupported key.")]
    UnsupportedKey,
    #[error("Failed to verify signed package '{}' because content is revoked.", .0)]
    ContentRevoked(String),
    #[cfg(feature = "aws-kms")]
    #[error("Failed to retrieve the public key from AWS KMS.")]
    AwsKmsFailedToGetPublicKey(
        #[from]
        aws_sdk_kms::error::SdkError<
            aws_sdk_kms::operation::get_public_key::GetPublicKeyError,
            aws_smithy_runtime_api::client::orchestrator::HttpResponse,
        >,
    ),
    #[cfg(feature = "aws-kms")]
    #[error("Failed to sign data with AWS KMS.")]
    AwsKmsFailedToSign(
        #[from]
        aws_sdk_kms::error::SdkError<
            aws_sdk_kms::operation::sign::SignError,
            aws_smithy_runtime_api::client::orchestrator::HttpResponse,
        >,
    ),
}
