use rusqlite::{params, Connection};

use crate::{types::QueryResult, EngineError};

#[derive(Debug, Clone, PartialEq)]
pub struct GraphEdge {
    pub src_chunk: String,
    pub dst_chunk: String,
    pub weight: f32,
    pub edge_type: String,
}

pub fn replace_edges(conn: &mut Connection, edges: &[GraphEdge]) -> Result<(), EngineError> {
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM graph_edges", [])?;

    for edge in edges {
        tx.execute(
            r#"
            INSERT OR REPLACE INTO graph_edges (src_chunk, dst_chunk, weight, edge_type)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![edge.src_chunk, edge.dst_chunk, edge.weight, edge.edge_type],
        )?;
    }

    tx.commit()?;
    Ok(())
}

pub fn chunk_neighbors(
    conn: &Connection,
    chunk_id: &str,
    limit: usize,
) -> Result<Vec<QueryResult>, EngineError> {
    let mut stmt = conn.prepare(
        r#"
        SELECT c.id, c.doc_id, c.content, d.filename, c.page, e.weight
        FROM graph_edges e
        JOIN chunks c ON c.id = CASE
            WHEN e.src_chunk = ?1 THEN e.dst_chunk
            ELSE e.src_chunk
        END
        JOIN documents d ON d.id = c.doc_id
        WHERE e.src_chunk = ?1 OR e.dst_chunk = ?1
        ORDER BY e.weight DESC
        LIMIT ?2
        "#,
    )?;
    let rows = stmt.query_map(params![chunk_id, limit as i64], |row| {
        let weight = row.get::<_, f64>(5)? as f32;
        Ok(QueryResult {
            chunk_id: row.get(0)?,
            doc_id: row.get(1)?,
            content: row.get(2)?,
            filename: row.get(3)?,
            page: row.get::<_, Option<i64>>(4)?.map(|page| page as u32),
            score: weight,
            score_bm25: 0.0,
            score_vec: 0.0,
            score_graph: weight,
        })
    })?;

    let mut results = Vec::new();
    for row in rows {
        results.push(row?);
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder_compiles() {
        assert_eq!(2 + 2, 4);
    }
}
