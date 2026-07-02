//! ripple-conversation-store: SQLite 持久化层。
//!
//! 对话/消息 CRUD、FTS5 全文搜索、Provider 配置、插件注册表、设置 KV。

mod db;
mod error;
mod message_repo;
mod migration;

pub mod conversation_repo;

pub use db::{init_db, DbPool};
pub use error::{StoreError, StoreResult};
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

        let updated = ConversationRepo::update(&conn, &conv.id, Some("Renamed"), None, None, None, None, None).unwrap();
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

        // 正序：msg 0, 1, 2, 3, 4
        let page1 = MessageRepo::list_by_conversation(&conn, &conv.id, 2, None).unwrap();
        assert_eq!(page1.len(), 2);
        assert_eq!(page1[0].text(), "msg 0");

        // 游标：取 msg 2 之前的
        let page2 = MessageRepo::list_by_conversation(&conn, &conv.id, 2, Some(&ids[2])).unwrap();
        assert_eq!(page2.len(), 2);
        assert_eq!(page2[0].text(), "msg 0");
        assert_eq!(page2[1].text(), "msg 1");
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
