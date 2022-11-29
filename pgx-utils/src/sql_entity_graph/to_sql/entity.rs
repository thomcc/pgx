/*
Portions Copyright 2019-2021 ZomboDB, LLC.
Portions Copyright 2021-2022 Technology Concepts & Design, Inc. <support@tcdi.com>

All rights reserved.

Use of this source code is governed by the MIT license that can be found in the LICENSE file.
*/
/*!

`sql = ...` fragment related entities for Rust to SQL translation

> Like all of the [`sql_entity_graph`][crate::sql_entity_graph] APIs, this is considered **internal**
to the `pgx` framework and very subject to change between versions. While you may use this, please do it with caution.

*/
use crate::sql_entity_graph::pgx_sql::PgxSql;
use crate::sql_entity_graph::to_sql::ToSqlFn;
use crate::sql_entity_graph::SqlGraphEntity;

/// Represents configuration options for tuning the SQL generator.
///
/// When an item that can be rendered to SQL has these options at hand, they should be
/// respected. If an item does not have them, then it is not expected that the SQL generation
/// for those items can be modified.
///
/// The default configuration has `enabled` set to `true`, and `callback` to `None`, which indicates
/// that the default SQL generation behavior will be used. These are intended to be mutually exclusive
/// options, so `callback` should only be set if generation is enabled.
///
/// When `enabled` is false, no SQL is generated for the item being configured.
///
/// When `callback` has a value, the corresponding `ToSql` implementation should invoke the
/// callback instead of performing their default behavior.
#[derive(Default, Clone)]
pub struct ToSqlConfigEntity {
    pub enabled: bool,
    pub callback: Option<ToSqlFn>,
    pub content: Option<&'static str>,
}
impl ToSqlConfigEntity {
    /// Helper used to implement traits (`Eq`, `Ord`, etc) despite `ToSqlFn` not
    /// having an implementation for them.
    #[inline]
    fn fields(&self) -> (bool, Option<&str>, Option<usize>) {
        (self.enabled, self.content, self.callback.map(|f| f as usize))
    }

    /// Given a SqlGraphEntity, this function converts it to SQL based on the current configuration.
    ///
    /// If the config overrides the default behavior (i.e. using the `ToSql` trait), then `Some(eyre::Result)`
    /// is returned. If the config does not override the default behavior, then `None` is returned. This can
    /// be used to dispatch SQL generation in a single line, e.g.:
    ///
    /// ```rust,ignore
    /// config.to_sql(entity, context).unwrap_or_else(|| entity.to_sql(context))?
    /// ```
    pub fn to_sql(
        &self,
        entity: &SqlGraphEntity,
        context: &PgxSql,
    ) -> Option<eyre::Result<String>> {
        use eyre::{eyre, WrapErr};

        if !self.enabled {
            return Some(Ok(format!(
                "\n\
                {sql_anchor_comment}\n\
                -- Skipped due to `#[pgx(sql = false)]`\n",
                sql_anchor_comment = entity.sql_anchor_comment(),
            )));
        }

        if let Some(content) = self.content {
            let module_pathname = context.get_module_pathname();

            let content = content.replace("@MODULE_PATHNAME@", &module_pathname);

            return Some(Ok(format!(
                "\n\
                {sql_anchor_comment}\n\
                {content}\n\
            ",
                content = content,
                sql_anchor_comment = entity.sql_anchor_comment()
            )));
        }

        if let Some(callback) = self.callback {
            let content = callback(entity, context)
                .map_err(|e| eyre!(e))
                .wrap_err("Failed to run specified `#[pgx(sql = path)] function`");
            return match content {
                Ok(content) => {
                    let module_pathname = &context.get_module_pathname();

                    let content = content.replace("@MODULE_PATHNAME@", &module_pathname);

                    Some(Ok(format!(
                        "\n\
                        {sql_anchor_comment}\n\
                        {content}\
                    ",
                        content = content,
                        sql_anchor_comment = entity.sql_anchor_comment(),
                    )))
                }
                Err(e) => Some(Err(e)),
            };
        }

        None
    }
}

impl Eq for ToSqlConfigEntity {}
impl PartialEq for ToSqlConfigEntity {
    fn eq(&self, other: &Self) -> bool {
        self.fields() == other.fields()
    }
}

impl Ord for ToSqlConfigEntity {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.fields().cmp(&other.fields())
    }
}

impl PartialOrd for ToSqlConfigEntity {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.fields().cmp(&other.fields()))
    }
}

impl std::hash::Hash for ToSqlConfigEntity {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.fields().hash(state)
    }
}

impl std::fmt::Debug for ToSqlConfigEntity {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let (enabled, content, callback) = self.fields();
        f.debug_struct("ToSqlConfigEntity")
            .field("enabled", &enabled)
            .field("callback", &callback)
            .field("content", &content)
            .finish()
    }
}
