// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::{v1_routes, Data};
use anyhow::Result;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{Response, StatusCode};
use axum::middleware::Next;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{middleware, Router};
use criticaltrust::manifests::{
    ManifestVersion, Package, PackageFile, PackageManifest, Release, ReleaseArtifact,
    ReleaseArtifactFormat, ReleaseManifest, ReleasePackage,
};
use criticaltrust::signatures::SignedPayload;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
#[cfg(unix)]
use std::os::unix::prelude::MetadataExt;
#[cfg(windows)]
use std::os::windows::prelude::MetadataExt;
use std::path::Path;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{MappedMutexGuard, Mutex, MutexGuard};
use tokio::task::JoinHandle;
use walkdir::WalkDir;
use xz2::write::XzEncoder;

pub struct MockServer {
    data: Arc<Mutex<Data>>,
    address: SocketAddr,
    handle: Option<JoinHandle<()>>,
}

impl MockServer {
    pub(crate) async fn spawn(data: Data) -> Self {
        let data = Arc::new(Mutex::new(data));
        let data_clone = data.clone();

        // Binding on port 0 results in the operative system picking a random available port,
        // without the need of generating a random port ourselves and validating the port is not
        // being used by another process.
        //
        // The real port can be then retrieved by checking the address of the bound server.
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0);
        let listener = TcpListener::bind(&address).await.unwrap();
        let address = listener.local_addr().unwrap();

        let mut this = Self {
            data: data_clone,
            address,
            handle: None,
        };
        let router = Router::new()
            .route("/", get(StatusCode::IM_A_TEAPOT))
            .route("/health", get(StatusCode::IM_A_TEAPOT))
            .nest("/v1", v1_routes())
            .layer(middleware::from_fn_with_state(
                Arc::clone(&data),
                update_history,
            ))
            .fallback(StatusCode::NOT_FOUND)
            .with_state(Arc::clone(&data));

        let handle = tokio::spawn(async move {
            axum::serve(listener, router)
                .await
                .expect("failed to start server");
        });

        this.handle = Some(handle);

        this
    }

    pub fn url(&self) -> String {
        format!("http://{}", self.address)
    }

    pub async fn served_requests_count(&self) -> usize {
        self.data.lock().await.history.len()
    }

    pub async fn history(&self) -> MappedMutexGuard<'_, [(Request<Body>, Response<Body>)]> {
        let guard = self.data.lock().await;
        MutexGuard::map(guard, |v| v.history.as_mut_slice())
    }

    pub async fn edit_data(&self, f: impl FnOnce(MutexGuard<'_, Data>)) {
        f(self.data.lock().await);
    }

    /// Creates a package, signs it and then tarballs it.
    ///
    /// This method is to be called within the test as many times as the number of packages
    /// to be generated.
    ///
    /// `package_name`: Name of the package. Keep it unique please.
    /// `product_name`: Name of the product, generally "ferrocene".
    /// `input_dir`: Path to the directory where package data is kept.
    /// `output_dir`: Path to the directory where output will be stored
    pub async fn create_package(
        &mut self,
        package_name: &str,
        product_name: &str,
        input_dir: &Path,
        output_dir: &Path,
    ) -> Result<(), ()> {
        let mut package = Package {
            product: product_name.to_string(),
            package: package_name.to_string(),
            commit: "123abc".to_string(),
            files: vec![],
            managed_prefixes: vec![],
        };

        collect_files(&mut package, input_dir);
        package.files.sort_by_cached_key(|file| file.path.clone());

        let mut signed = SignedPayload::new(&package).unwrap();
        let keypair = {
            let keypair_lock = self.data.lock().await;
            keypair_lock.keypairs.get("packages").unwrap().clone()
        };
        signed.add_signature(&keypair).await.unwrap();

        let package_manifest_with_dir_structure = input_dir
            .join("share")
            .join("criticaltrust")
            .join(product_name);
        tokio::fs::create_dir_all(&package_manifest_with_dir_structure)
            .await
            .unwrap();
        tokio::fs::write(
            package_manifest_with_dir_structure.join(format!("{}.json", package_name)),
            serde_json::to_vec_pretty(&PackageManifest {
                version: ManifestVersion::<1>,
                signed,
            })
            .unwrap(),
        )
        .await
        .unwrap();

        let archive_name = format!("{}.tar.xz", package_name);
        let output_compressed_file = File::create(output_dir.join(&archive_name)).unwrap();
        let encoder = XzEncoder::new(output_compressed_file, 9);
        let mut tar = tar::Builder::new(encoder);
        tar.append_dir_all("", input_dir).unwrap();
        tar.into_inner().unwrap().finish().unwrap();

        Ok(())
    }

    /// Create a signed release.
    ///
    /// ** Use `Self::create_package()` before calling this method. **
    ///
    /// `product_name`: Name of the product, generally "ferrocene".
    /// `release_name`: Release version, usually a string like "25.02.0".
    /// `packages`: Vec of package names. It is important that these packages exist inside the
    ///             output directory.
    ///             Use `Self::create_package()` before calling this method.
    /// `output_dir`: Path to the directory where output will be stored
    ///                 (and output of `Self::create_package()` was previously stored.
    pub async fn create_release(
        &mut self,
        product_name: &str,
        release_name: &str,
        packages: Vec<&str>,
        output_dir: &Path,
    ) -> Result<(), ()> {
        let release_manifest = output_dir.join("criticalup-release-manifest.json");
        let mut packages_update: Vec<ReleasePackage> = vec![];

        // Create a `ReleasePackage` for each package in the vec. This is needed because we
        // expect only package names.
        for item in packages {
            let artifact_file = std::fs::read(output_dir.join(format!("{}.tar.xz", item))).unwrap();
            let artifact_file_metadata =
                std::fs::metadata(output_dir.join(format!("{}.tar.xz", item))).unwrap();

            let mut hasher = Sha256::new();
            hasher.update(&artifact_file);
            let hash = hasher.finalize().to_vec();

            let artifact = ReleaseArtifact {
                format: ReleaseArtifactFormat::TarXz,
                #[cfg(not(windows))]
                size: artifact_file_metadata.size() as usize,
                #[cfg(windows)]
                size: artifact_file_metadata.file_size() as usize,
                sha256: hash,
            };
            packages_update.push(ReleasePackage {
                package: item.to_string(),
                artifacts: vec![artifact],
                dependencies: vec![],
            });

            {
                let mut data_grabbed = self.data.lock().await;
                data_grabbed.release_packages.insert(
                    (
                        product_name.to_string(),
                        release_name.to_string(),
                        item.to_string(),
                    ),
                    artifact_file,
                );
            }
        }

        let mut signed = SignedPayload::new(&Release {
            product: product_name.to_string(),
            release: release_name.to_string(),
            commit: "123abc".to_string(),
            packages: packages_update,
        })
        .unwrap();
        let keypair = {
            let keypair_lock = self.data.lock().await;
            keypair_lock.keypairs.get("releases").unwrap().clone()
        };
        signed.add_signature(&keypair).await.unwrap();

        let release_manifest_content = &ReleaseManifest {
            version: ManifestVersion,
            signed,
        };

        tokio::fs::write(
            &release_manifest,
            serde_json::to_vec_pretty(release_manifest_content).unwrap(),
        )
        .await
        .unwrap();

        {
            let mut data_grabbed = self.data.lock().await;
            data_grabbed.release_manifests.insert(
                (product_name.to_string(), release_name.to_string()),
                release_manifest_content.clone(),
            );
        }

        Ok(())
    }
}

fn collect_files(package: &mut Package, dir: &Path) {
    for entry in WalkDir::new(dir) {
        let entry = entry.unwrap();
        let relative_path = entry.path().strip_prefix(dir).unwrap();
        if entry.file_type().is_file() {
            package.files.push(PackageFile {
                path: relative_path.into(),
                #[cfg(not(windows))]
                posix_mode: entry.metadata().unwrap().mode(),
                #[cfg(windows)]
                posix_mode: 0,
                sha256: hash_file(entry.path()),
                needs_proxy: false,
            })
        } else if entry.file_type().is_file() {
            collect_files(package, entry.path());
        }
    }
}

fn hash_file(path: &Path) -> Vec<u8> {
    let mut sha256 = Sha256::new();
    let mut contents = File::open(path).unwrap();
    std::io::copy(&mut contents, &mut sha256).unwrap();
    sha256.finalize().to_vec()
}

async fn update_history(
    State(data): State<Arc<Mutex<Data>>>,
    req: Request,
    next: Next,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    // Body does not implment Clone, that would be inefficient.
    // But, we're in a test, and having a history is actually very useful.

    let (req_parts, req_body) = req.into_parts();
    let body_bytes = axum::body::to_bytes(req_body, usize::MAX).await.unwrap();
    eprintln!("-> {} {}", req_parts.method, req_parts.uri);
    let req_for_history = Request::from_parts(req_parts.clone(), Body::from(body_bytes.clone()));
    let req_for_next = Request::from_parts(req_parts, Body::from(body_bytes));

    let res = next.run(req_for_next).await;

    let (res_parts, res_body) = res.into_parts();
    let body_bytes = axum::body::to_bytes(res_body, usize::MAX).await.unwrap();
    eprintln!(
        "<- {} {}",
        res_parts.status,
        String::from_utf8(body_bytes.to_vec()).unwrap_or_else(|_| "<bytes>".to_string())
    );
    let res_for_history = Response::from_parts(res_parts.clone(), Body::from(body_bytes.clone()));
    let res_for_next = Response::from_parts(res_parts, Body::from(body_bytes));

    data.lock()
        .await
        .history
        .push((req_for_history, res_for_history));

    Ok(res_for_next)
}
