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
    #[error("failed to sign data")]
    SignatureFailed,
    #[error("failed to verify signed data")]
    VerificationFailed,
    #[error("failed to generate a local key")]
    LocalKeyGenerationFailed,
    #[error("wrong key role for the trust root key (expected Root, found {0:?})")]
    WrongKeyRoleForTrustRoot(KeyRole),
    #[error("failed to serialize the contents of the signed payload")]
    SignedPayloadSerializationFailed(#[source] serde_json::Error),
    #[error("failed to deserialize verified data")]
    DeserializationFailed(#[source] serde_json::Error),
    #[error("failed to load key pair")]
    InvalidKey(String),
    #[error("unsupported key")]
    UnsupportedKey,
    #[error("verification failed because the signatures expired")]
    SignaturesExpired,
    #[error("failed to verify signed data because payload is revoked")]
    ContentRevoked,
    #[error("failed to calculate SHA256 from payload")]
    PayloadSha256CalcFailed(#[source] FromUtf8Error),
    #[cfg(feature = "aws-kms")]
    #[error("failed to retrieve the public key from AWS KMS")]
    AwsKmsFailedToGetPublicKey(
        #[from]
        aws_sdk_kms::error::SdkError<
            aws_sdk_kms::operation::get_public_key::GetPublicKeyError,
            aws_smithy_runtime_api::client::orchestrator::HttpResponse,
        >,
    ),
    #[cfg(feature = "aws-kms")]
    #[error("failed to sign data with AWS KMS")]
    AwsKmsFailedToSign(
        #[from]
        aws_sdk_kms::error::SdkError<
            aws_sdk_kms::operation::sign::SignError,
            aws_smithy_runtime_api::client::orchestrator::HttpResponse,
        >,
    ),
}
