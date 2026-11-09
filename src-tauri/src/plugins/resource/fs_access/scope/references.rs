use super::*;

impl Scope {
    pub fn resource_bound(&self, reference: &Value) -> Result<bool, String> {
        let key = Self::ref_key(reference)?;
        let issued = self.issued_reference_active(&key)?;
        Ok(self.references.contains_key(&key)
            || self.metadata_refs.contains_key(&key)
            || self.remote_resources.contains_key(&key)
            || self
                .mapped_resources
                .lock()
                .map_err(|_| "mapped resource registry unavailable")?
                .contains_key(&key)
            || issued)
    }
    pub fn bind_resource(&mut self, resource: &Value, path: Option<&Path>) -> Result<(), String> {
        let key = Self::ref_key(&resource["ref"])?;
        if let Some(path) = path {
            let path = canonical_selection(path)?.to_string_lossy().into_owned();
            self.authorize(&path)?;
            std::sync::Arc::make_mut(&mut self.references).insert(key.clone(), path.clone());
        }
        if let Some(metadata) = resource.get("metadata").filter(|v| v.is_object()) {
            let mut metadata = metadata.clone();
            metadata["isDir"] = json!(metadata["kind"] == "directory");
            metadata["path"] = resource["ref"]["nodeId"].clone();
            for (key, value) in [
                ("size", json!(0)),
                ("modified", json!(0)),
                ("extension", json!("")),
                ("mime", json!("")),
                ("symlink", json!(false)),
            ] {
                if metadata.get(key).is_none_or(Value::is_null) {
                    metadata[key] = value;
                }
            }
            std::sync::Arc::make_mut(&mut self.metadata_refs).insert(key, metadata);
        }
        Ok(())
    }
    pub fn bind_remote(
        &mut self,
        resource: &Value,
        snapshot: &crate::plugins::resource::resource_context::ResourceSnapshot,
    ) -> Result<(), String> {
        std::sync::Arc::make_mut(&mut self.remote_resources)
            .insert(Self::ref_key(&resource["ref"])?, snapshot.clone());
        Ok(())
    }
    pub fn remote_target(
        &self,
        p: &Value,
    ) -> Result<Option<crate::plugins::resource::resource_context::ResourceSnapshot>, String> {
        if let Some(reference) = p.get("ref") {
            let key = Self::ref_key(reference)?;
            if let Some(snapshot) = self
                .mapped_resources
                .lock()
                .ok()
                .and_then(|mapped| mapped.get(&key).map(|binding| binding.snapshot.clone()))
            {
                return Ok(Some(snapshot));
            }
            if let Some(snapshot) = self.remote_resources.get(&key) {
                return Ok(Some(snapshot.clone()));
            }
            if !self.issued_reference_active(&key)? {
                return Ok(None);
            }
            let issued = self
                .issued
                .lock()
                .map_err(|_| "reference registry unavailable")?;
            return Ok(issued.get(&key).and_then(|snapshot| {
                crate::core::fs::provider_registry::resource_access(&snapshot.display)
                    .is_ok_and(|access| {
                        access == crate::core::fs::provider_registry::ResourceAccess::Provider
                    })
                    .then(|| snapshot.clone())
            }));
        }
        Ok(None)
    }
    /// Resolve only identities already bound to this exact operation. A child
    /// cannot use an arbitrary parent path to acquire a different mount/root.
    pub fn reference_for_path(&self, path: &str) -> Result<Value, String> {
        if let Some((key, _)) = self.references.iter().find(|(_, p)| p.as_str() == path) {
            return serde_json::from_str(key).map_err(|e| e.to_string());
        }
        if let Some((key, _)) = self
            .remote_resources
            .iter()
            .find(|(_, r)| r.display == path)
        {
            return serde_json::from_str(key).map_err(|e| e.to_string());
        }
        if let Some((key, _)) = self
            .mapped_resources
            .lock()
            .map_err(|_| "mapped resource registry unavailable")?
            .iter()
            .find(|(_, binding)| binding.path == path || binding.snapshot.display == path)
        {
            return serde_json::from_str(key).map_err(|e| e.to_string());
        }
        let issued_key = {
            let issued = self
                .issued
                .lock()
                .map_err(|_| "reference registry unavailable")?;
            issued
                .iter()
                .find(|(_, resource)| resource.display == path)
                .map(|(key, _)| key.clone())
        };
        if let Some(key) = issued_key
            && self.issued_reference_active(&key)?
        {
            return serde_json::from_str(&key).map_err(|e| e.to_string());
        }
        Err("ScopeDenied: parent directory is not bound".into())
    }
    pub fn metadata_reference(&self, reference: &Value) -> Result<Option<&Value>, String> {
        Ok(self.metadata_refs.get(&Self::ref_key(reference)?))
    }
    pub fn resolve_reference(&self, reference: &Value) -> Result<String, String> {
        let key = Self::ref_key(reference)?;
        if let Some(path) = self.references.get(&key) {
            return Ok(path.clone());
        }
        if let Some(path) = self
            .mapped_resources
            .lock()
            .map_err(|_| "mapped resource registry unavailable")?
            .get(&key)
            .map(|binding| binding.path.clone())
        {
            return Ok(path);
        }
        if self.issued_reference_active(&key)?
            && let Some(display) = self
                .issued
                .lock()
                .map_err(|_| "reference registry unavailable")?
                .get(&key)
                .map(|snapshot| snapshot.display.clone())
        {
            return Ok(display);
        }
        Err("resource not authorized in this operation".into())
    }
    /// Mint a child identity into the shared operation registry. Cloning a
    /// Scope into a task preserves issued refs but never widens its selection.
    pub fn issue_child(
        &self,
        parent: &Value,
        display: &str,
        metadata: Option<crate::plugins::contracts::ResourceMetadata>,
    ) -> Result<Value, String> {
        let parent_path = self.resolve_reference(parent).or_else(|_| {
            self.remote_target(&json!({"ref":parent}))?
                .map(|snapshot| snapshot.display)
                .ok_or_else(|| "parent ref is not authorized".to_string())
        })?;
        crate::core::fs::provider_registry::validate_child(self, &parent_path, display)?;
        let meta =
            metadata
                .clone()
                .unwrap_or_else(|| crate::plugins::contracts::ResourceMetadata {
                    name: Path::new(display)
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    kind: "file".into(),
                    ..Default::default()
                });
        let snapshot = crate::plugins::resource::resource_context::from_metadata(display, meta);
        let reference =
            serde_json::to_value(&snapshot.reference).map_err(|error| error.to_string())?;
        let child_key = Self::ref_key(&reference)?;
        let parent_key = Self::ref_key(parent)?;
        if !self
            .issued_capabilities
            .lock()
            .map_err(|_| "reference capability registry unavailable")?
            .derive(&parent_key, &child_key)
        {
            return Err("child reference was revoked or rebound to another parent".into());
        }
        self.issued
            .lock()
            .map_err(|_| "reference registry unavailable")?
            .insert(child_key, snapshot);
        Ok(json!({"ref":reference,"metadata":metadata}))
    }
}
