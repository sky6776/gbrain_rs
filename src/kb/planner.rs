//! Query Planner — 按查询类型选择检索策略和权重 (P3-006, P3-007)
//!
//! 基于规则的初版分类器，后续可接入 ML 模型。

use serde::{Deserialize, Serialize};

/// 查询类型分类
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueryType {
    /// 精确查找：编号、代码、短关键词
    ExactLookup,
    /// 流程/操作指南："怎么/如何/流程/步骤"
    HowTo,
    /// 事实查询："是什么/定义/概念"
    FactLookup,
    /// 概念/开放式搜索
    Conceptual,
    /// 表格查询：含 "sheet/表/清单/xlsx/csv"
    TableLookup,
    /// 时间范围查询：含日期/时间词
    RecentOrTimebound,
    /// 小文档优先
    SmallDocument,
}

impl QueryType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ExactLookup => "exact_lookup",
            Self::HowTo => "how_to",
            Self::FactLookup => "fact_lookup",
            Self::Conceptual => "conceptual",
            Self::TableLookup => "table_lookup",
            Self::RecentOrTimebound => "recent_or_timebound",
            Self::SmallDocument => "small_document",
        }
    }
}

/// Retriever 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RetrieverType {
    TitleName,
    NodeFts,
    Vector,
    Summary,
    Table,
    Metadata,
}

/// Planner 输出：retriever 组合 + 权重
#[derive(Debug, Clone)]
pub struct PlannerOutput {
    pub query_type: QueryType,
    pub retrievers: Vec<(RetrieverType, f64)>,
}

/// 基于规则识别查询类型
pub fn classify_query(query: &str) -> QueryType {
    let q = query.trim().to_lowercase();

    // 表格查询模式
    if q.contains("表")
        || q.contains("sheet")
        || q.contains("清单")
        || q.contains("xlsx")
        || q.contains("csv")
        || q.contains("汇总")
    {
        return QueryType::TableLookup;
    }

    // 流程/操作指南
    if q.contains("怎么") || q.contains("如何") || q.contains("流程") || q.contains("步骤")
    {
        return QueryType::HowTo;
    }

    // 时间范围
    if q.contains("年")
        || q.contains("月")
        || q.contains("日")
        || q.contains("最近")
        || q.contains("去年")
        || q.contains("上季度")
        || q.contains("q1")
        || q.contains("q2")
    {
        return QueryType::RecentOrTimebound;
    }

    // 事实查询
    if q.contains("是什么") || q.contains("定义") || q.ends_with('?') || q.ends_with('？') {
        return QueryType::FactLookup;
    }

    // 精确查找：短 query (< 8 chars) 且包含编号/代码模式
    if q.chars().count() < 8 {
        return QueryType::ExactLookup;
    }

    QueryType::Conceptual
}

/// 根据查询类型输出 retriever 组合和权重
pub fn plan(query_type: QueryType) -> PlannerOutput {
    let retrievers = match query_type {
        QueryType::ExactLookup => vec![
            (RetrieverType::TitleName, 0.6),
            (RetrieverType::Metadata, 0.2),
            (RetrieverType::NodeFts, 0.2),
        ],
        QueryType::HowTo => vec![
            (RetrieverType::NodeFts, 0.3),
            (RetrieverType::Vector, 0.3),
            (RetrieverType::Summary, 0.2),
            (RetrieverType::TitleName, 0.2),
        ],
        QueryType::FactLookup => vec![
            (RetrieverType::NodeFts, 0.4),
            (RetrieverType::Vector, 0.3),
            (RetrieverType::Metadata, 0.3),
        ],
        QueryType::Conceptual => vec![
            (RetrieverType::Vector, 0.5),
            (RetrieverType::NodeFts, 0.3),
            (RetrieverType::Summary, 0.2),
        ],
        QueryType::TableLookup => vec![
            (RetrieverType::Table, 0.5),
            (RetrieverType::NodeFts, 0.3),
            (RetrieverType::Metadata, 0.2),
        ],
        QueryType::RecentOrTimebound => vec![
            (RetrieverType::NodeFts, 0.4),
            (RetrieverType::Metadata, 0.4),
            (RetrieverType::Vector, 0.2),
        ],
        QueryType::SmallDocument => vec![
            (RetrieverType::TitleName, 0.4),
            (RetrieverType::NodeFts, 0.3),
            (RetrieverType::Vector, 0.3),
        ],
    };

    PlannerOutput {
        query_type,
        retrievers,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_how_to() {
        assert_eq!(classify_query("差旅报销怎么申请"), QueryType::HowTo);
        assert_eq!(classify_query("如何配置SSL证书"), QueryType::HowTo);
    }

    #[test]
    fn test_classify_table() {
        assert_eq!(classify_query("2024 Q3 报销清单"), QueryType::TableLookup);
        assert_eq!(classify_query("导出xlsx报表"), QueryType::TableLookup);
    }

    #[test]
    fn test_classify_exact() {
        assert_eq!(classify_query("ABC-123"), QueryType::ExactLookup);
    }

    #[test]
    fn test_classify_conceptual() {
        assert_eq!(
            classify_query("机器学习在自然语言处理中的应用"),
            QueryType::Conceptual
        );
    }

    #[test]
    fn test_plan_output() {
        let plan = plan(QueryType::ExactLookup);
        assert_eq!(plan.retrievers.len(), 3);
        assert!((plan.retrievers[0].1 - 0.6).abs() < 0.01);
    }
}
