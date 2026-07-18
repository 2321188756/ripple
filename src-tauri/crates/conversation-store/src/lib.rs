//! ripple-conversation-store: SQLite 持久化层。
//!
//! 对话/消息 CRUD、FTS5 全文搜索、Provider 配置、插件注册表、设置 KV。

mod db;
mod error;
mod memory_repo;
mod message_repo;
mod migration;

pub mod conversation_repo;

pub use db::{init_db, DbPool};
pub use error::{StoreError, StoreResult};
pub use memory_repo::{MemoryChunk, MemoryFileMeta, MemoryRepo};
pub use message_repo::{MessageRepo, SearchResult};

#[cfg(test)]
mod tests {
    use ripple_core::{Message, MessageRole};

    use crate::conversation_repo::ConversationRepo;
    use crate::db::init_memory_db;
    use crate::message_repo::MessageRepo;

    fn setup() -> crate::DbPool {
        init_memory_db().unwrap()
    }

    #[test]
    fn create_and_get_conversation() {
        let pool = setup();
        let conn = pool.get().unwrap();
        let conv = ConversationRepo::create(&conn, "openai", "gpt-4o", Some("Test"), None).unwrap();
        assert_eq!(conv.title, "Test");
        assert_eq!(conv.provider_id, "openai");

        let fetched = ConversationRepo::get_by_id(&conn, &conv.id).unwrap();
        assert_eq!(fetched.id, conv.id);
        assert_eq!(fetched.title, "Test");
    }

    #[test]
    fn list_conversations_with_search() {
        let pool = setup();
        let conn = pool.get().unwrap();
        ConversationRepo::create(&conn, "openai", "gpt-4o", Some("Hello World"), None).unwrap();
        ConversationRepo::create(&conn, "anthropic", "claude-3", Some("Test Chat"), None).unwrap();

        let all = ConversationRepo::list(&conn, None, 10, 0).unwrap();
        assert_eq!(all.len(), 2);

        let searched = ConversationRepo::list(&conn, Some("Hello"), 10, 0).unwrap();
        assert_eq!(searched.len(), 1);
        assert_eq!(searched[0].title, "Hello World");
    }

    #[test]
    fn update_and_delete_conversation() {
        let pool = setup();
        let conn = pool.get().unwrap();
        let conv = ConversationRepo::create(&conn, "openai", "gpt-4o", None, None).unwrap();

        let updated = ConversationRepo::update(
            &conn,
            &conv.id,
            Some("Renamed"),
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
        assert_eq!(updated.title, "Renamed");

        ConversationRepo::delete(&conn, &conv.id).unwrap();
        assert!(ConversationRepo::get_by_id(&conn, &conv.id).is_err());
    }

    #[test]
    fn insert_and_list_messages() {
        let pool = setup();
        let conn = pool.get().unwrap();
        let conv = ConversationRepo::create(&conn, "openai", "gpt-4o", None, None).unwrap();

        let msg = Message::new_user(&conv.id, "hello");
        MessageRepo::insert(&conn, &msg).unwrap();

        let msgs = MessageRepo::list_by_conversation(&conn, &conv.id, 10, None).unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].text(), "hello");
        assert_eq!(msgs[0].role, MessageRole::User);
    }

    #[test]
    fn message_pagination_by_cursor() {
        let pool = setup();
        let conn = pool.get().unwrap();
        let conv = ConversationRepo::create(&conn, "openai", "gpt-4o", None, None).unwrap();

        let ids: Vec<String> = (0..5)
            .map(|i| {
                let m = Message::new_user(&conv.id, &format!("msg {i}"));
                MessageRepo::insert(&conn, &m).unwrap();
                m.id
            })
            .collect();

        // 首页返回最新两条，但页内仍按时间正序展示。
        let page1 = MessageRepo::list_by_conversation(&conn, &conv.id, 2, None).unwrap();
        assert_eq!(page1.len(), 2);
        assert_eq!(page1[0].text(), "msg 3");
        assert_eq!(page1[1].text(), "msg 4");

        // 游标页返回紧邻游标之前的两条。
        let page2 = MessageRepo::list_by_conversation(&conn, &conv.id, 2, Some(&ids[3])).unwrap();
        assert_eq!(page2.len(), 2);
        assert_eq!(page2[0].text(), "msg 1");
        assert_eq!(page2[1].text(), "msg 2");
    }

    #[test]
    fn deletion_semantics_are_explicit() {
        let pool = setup();
        let conn = pool.get().unwrap();
        let conv = ConversationRepo::create(&conn, "openai", "gpt-4o", None, None).unwrap();
        let messages: Vec<_> = (0..4)
            .map(|i| {
                let message = Message::new_user(&conv.id, &format!("msg {i}"));
                MessageRepo::insert(&conn, &message).unwrap();
                message
            })
            .collect();

        assert_eq!(
            MessageRepo::truncate_after(&conn, &conv.id, &messages[1].id).unwrap(),
            2
        );
        let remaining = MessageRepo::list_by_conversation(&conn, &conv.id, 10, None).unwrap();
        assert_eq!(remaining.len(), 2);
        assert_eq!(remaining[1].id, messages[1].id);
        assert_eq!(
            MessageRepo::delete_from_inclusive(&conn, &conv.id, &messages[1].id).unwrap(),
            1
        );
        assert!(MessageRepo::truncate_after(&conn, &conv.id, "missing").is_err());
    }

    #[test]
    fn duplicate_message_ids_fail() {
        let pool = setup();
        let conn = pool.get().unwrap();
        let conv = ConversationRepo::create(&conn, "openai", "gpt-4o", None, None).unwrap();
        let message = Message::new_user(&conv.id, "hello");
        MessageRepo::insert(&conn, &message).unwrap();
        assert!(MessageRepo::insert(&conn, &message).is_err());
    }

    #[test]
    fn search_messages_via_fts() {
        let pool = setup();
        let conn = pool.get().unwrap();
        let conv = ConversationRepo::create(&conn, "openai", "gpt-4o", None, None).unwrap();

        let m = Message::new_user(&conv.id, "the quick brown fox jumps over the lazy dog");
        MessageRepo::insert(&conn, &m).unwrap();

        let results = MessageRepo::search(&conn, "fox", None, 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].conversation_id, conv.id);
    }

    #[test]
    fn delete_conversation_cascades_messages() {
        let pool = setup();
        let conn = pool.get().unwrap();
        let conv = ConversationRepo::create(&conn, "openai", "gpt-4o", None, None).unwrap();

        for i in 0..3 {
            MessageRepo::insert(&conn, &Message::new_user(&conv.id, &format!("msg {i}"))).unwrap();
        }

        ConversationRepo::delete(&conn, &conv.id).unwrap();
        let msgs = MessageRepo::list_by_conversation(&conn, &conv.id, 10, None).unwrap();
        assert!(msgs.is_empty());
    }
}
