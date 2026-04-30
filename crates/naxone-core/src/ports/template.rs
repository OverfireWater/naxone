use crate::domain::vhost::VirtualHost;
use crate::error::Result;

/// Render configuration files from templates
pub trait TemplateEngine: Send + Sync {
    fn render_nginx_vhost(&self, vhost: &VirtualHost) -> Result<String>;
    fn render_apache_vhost(&self, vhost: &VirtualHost) -> Result<String>;
}
