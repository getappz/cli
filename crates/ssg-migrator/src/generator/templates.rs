//! Page templates for Astro generation.

pub(super) fn generate_client_only_page(component_name: &str) -> String {
    format!(
        r#"---
import Layout from '../layouts/Layout.astro';
import {comp} from '../components/ui/{comp}.tsx';
---
<Layout>
  <{comp} client:only="react" />
</Layout>
"#,
        comp = component_name
    )
}
