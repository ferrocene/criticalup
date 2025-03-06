// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::handlers::handle_request;
use crate::Data;
use criticaltrust::keys::KeyPair;
use criticaltrust::manifests::{ManifestVersion, Package, PackageFile, PackageManifest};
use criticaltrust::signatures::SignedPayload;
use sha2::{Digest, Sha256};
use std::fs::File;
#[cfg(unix)]
use std::os::unix::prelude::MetadataExt;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use tiny_http::Server;

pub struct MockServer {
    data: Arc<Mutex<Data>>,
    server: Arc<Server>,
    handle: Option<JoinHandle<()>>,
    served_requests: Arc<AtomicUsize>,
}

impl MockServer {
    pub(crate) fn spawn(data: Data) -> Self {
        let data = Arc::new(Mutex::new(data));

        // Binding on port 0 results in the operative system picking a random available port,
        // without the need of generating a random port ourselves and validating the port is not
        // being used by another process.
        //
        // The real port can be then retrieved by checking the address of the bound server.
        let server = Arc::new(Server::http("127.0.0.1:0").unwrap());

        let served_requests = Arc::new(AtomicUsize::new(0));

        let data_clone = data.clone();
        let server_clone = server.clone();
        let served_requests_clone = served_requests.clone();
        let handle = std::thread::spawn(move || {
            server_thread(data_clone, server_clone, served_requests_clone);
        });

        Self {
            data,
            server,
            handle: Some(handle),
            served_requests,
        }
    }

    pub fn url(&self) -> String {
        format!("http://{}", self.server.server_addr())
    }

    pub fn served_requests_count(&self) -> usize {
        self.served_requests.load(Ordering::SeqCst)
    }

    pub fn edit_data(&self, f: impl FnOnce(&mut Data)) {
        f(&mut self.data.lock().unwrap());
    }

    // Creates a package, signs it and then tarballs it.
    pub async fn create_package(
        &mut self,
        package_name: &str,
        product_name: &str,
        package_dir: &Path,
    ) -> Result<(), ()> {
        let mut package = Package {
            product: product_name.to_string(),
            package: package_name.to_string(),
            commit: "123abc".to_string(),
            files: vec![],
            managed_prefixes: vec![],
        };

        collect_files(&mut package, package_dir);

        package.files.sort_by_cached_key(|file| file.path.clone());
        let mut signed = SignedPayload::new(&package).unwrap();
        let keypair_lock = &self.data.lock().unwrap().keypair;
        signed
            .add_signature(keypair_lock.as_ref().unwrap())
            .await
            .unwrap();

        let dest_dir = package_dir
            .join("share")
            .join("criticaltrust")
            .join(product_name);
        std::fs::create_dir_all(&dest_dir).unwrap();
        std::fs::write(
            &dest_dir.join(format!("{}.json", package_name)),
            &serde_json::to_vec_pretty(&PackageManifest {
                version: ManifestVersion::<1>,
                signed,
            })
            .unwrap(),
        )
        .unwrap();

        // tarball it

        todo!()
    }

    pub fn create_release(&mut self, name: &str, packages: Vec<&str>) -> Result<(), ()> {
        todo!()
    }
}

impl Drop for MockServer {
    fn drop(&mut self) {
        self.server.unblock();
        if let Some(handle) = self.handle.take() {
            match handle.join() {
                Ok(_) => (),
                Err(err) => eprintln!("{err:?}"),
            }
        }
    }
}

fn server_thread(data: Arc<Mutex<Data>>, server: Arc<Server>, served_requests: Arc<AtomicUsize>) {
    for request in server.incoming_requests() {
        let response = handle_request(&data.lock().unwrap(), &request);
        request.respond(response).unwrap();

        served_requests.fetch_add(1, Ordering::SeqCst);
    }
}

fn collect_files(package: &mut Package, dir: &Path) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap().path();
        let relative_path = entry.strip_prefix(dir).unwrap();
        if entry.is_file() {
            package.files.push(PackageFile {
                path: relative_path.into(),
                #[cfg(not(windows))]
                posix_mode: entry.metadata().unwrap().mode(),
                #[cfg(windows)]
                posix_mode: 0,
                sha256: hash_file(&entry),
                needs_proxy: false,
            })
        } else if entry.is_dir() {
            collect_files(package, &entry);
        }
    }
}

fn hash_file(path: &Path) -> Vec<u8> {
    let mut sha256 = Sha256::new();
    let mut contents = File::open(path).unwrap();
    std::io::copy(&mut contents, &mut sha256).unwrap();
    sha256.finalize().to_vec()
}
