use super::*;

#[test]
fn project_roundtrip_camel_case() {
    let project = Project {
        id: "123".to_string(),
        name: "Platform".to_string(),
        description: None,
        created_at: "2026-04-14T12:00:00Z".to_string(),
        updated_at: Some("2026-04-14T13:00:00Z".to_string()),
    };

    let json = serde_json::to_value(&project).unwrap();
    assert_eq!(json["id"], "123");
    assert_eq!(json["createdAt"], "2026-04-14T12:00:00Z");
    assert_eq!(json["updatedAt"], "2026-04-14T13:00:00Z");
    assert!(json.get("created_at").is_none(), "should use camelCase");

    let deserialized: Project = serde_json::from_value(json).unwrap();
    assert_eq!(deserialized, project);
}

#[test]
fn card_deserialize_from_planka_api() {
    let api_json = serde_json::json!({
        "id": "1753741392678487554",
        "createdAt": "2026-04-15T11:51:05.476Z",
        "updatedAt": "2026-04-15T13:12:41.064Z",
        "type": "project",
        "position": 65536.0,
        "name": "PLNK-001: Workspace scaffolding",
        "description": "Some description",
        "dueDate": null,
        "isDueCompleted": null,
        "stopwatch": null,
        "commentsTotal": 0,
        "isClosed": false,
        "listChangedAt": "2026-04-15T13:12:41.062Z",
        "boardId": "1753741387376887253",
        "listId": "1753741388198970844",
        "creatorUserId": "1750688362236216321",
        "prevListId": null,
        "coverAttachmentId": null,
        "isSubscribed": false
    });

    let card: Card = serde_json::from_value(api_json).unwrap();
    assert_eq!(card.id, "1753741392678487554");
    assert_eq!(card.name, "PLNK-001: Workspace scaffolding");
    assert_eq!(card.board_id, "1753741387376887253");
    assert_eq!(card.list_id, "1753741388198970844");
    assert!(!card.is_closed);
    assert!(!card.is_subscribed);
    assert_eq!(card.description, Some("Some description".to_string()));
    assert!(card.due_date.is_none());
}

#[test]
fn board_roundtrip() {
    let board = Board {
        id: "456".to_string(),
        project_id: "123".to_string(),
        name: "Sprint".to_string(),
        position: 65536.0,
        created_at: "2026-04-14T12:00:00Z".to_string(),
        updated_at: None,
    };

    let json = serde_json::to_value(&board).unwrap();
    assert_eq!(json["projectId"], "123");
    let deserialized: Board = serde_json::from_value(json).unwrap();
    assert_eq!(deserialized, board);
}

#[test]
fn list_roundtrip() {
    let list = List {
        id: "789".to_string(),
        board_id: "456".to_string(),
        name: "Backlog".to_string(),
        position: 65536.0,
        color: None,
        created_at: "2026-04-14T12:00:00Z".to_string(),
        updated_at: None,
    };

    let json = serde_json::to_value(&list).unwrap();
    assert_eq!(json["boardId"], "456");
    let deserialized: List = serde_json::from_value(json).unwrap();
    assert_eq!(deserialized, list);
}

#[test]
fn list_deserialize_from_planka_api() {
    let api_json = serde_json::json!({
        "id": "1753741387863426522",
        "createdAt": "2026-04-15T11:51:04.902Z",
        "updatedAt": null,
        "type": "active",
        "position": 65536.0,
        "name": "Backlog",
        "color": null,
        "boardId": "1753741387376887253"
    });

    let list: List = serde_json::from_value(api_json).unwrap();
    assert_eq!(list.id, "1753741387863426522");
    assert_eq!(list.name, "Backlog");
    assert!(list.color.is_none());
}

#[test]
fn user_deserialize_from_list_endpoint() {
    let api_json = serde_json::json!({
        "id": "1750728282271122486",
        "createdAt": "2026-04-11T08:04:34.723Z",
        "updatedAt": "2026-04-12T01:12:19.850Z",
        "role": "projectOwner",
        "name": "Claude",
        "username": "claude",
        "phone": null,
        "organization": null,
        "isDeactivated": false,
        "avatar": null
    });

    let user: User = serde_json::from_value(api_json).unwrap();
    assert_eq!(user.id, "1750728282271122486");
    assert_eq!(user.name, "Claude");
    assert_eq!(user.username, Some("claude".to_string()));
    assert_eq!(user.role, "projectOwner");
    assert!(user.email.is_none());
    assert!(!user.is_deactivated);
}

#[test]
fn user_deserialize_from_me_endpoint() {
    let api_json = serde_json::json!({
        "id": "1750728282271122486",
        "createdAt": "2026-04-11T08:04:34.723Z",
        "updatedAt": "2026-04-12T01:12:19.850Z",
        "email": "test@example.com",
        "role": "projectOwner",
        "name": "Claude",
        "username": "claude",
        "phone": null,
        "organization": null,
        "language": "en-US",
        "isDeactivated": false,
        "avatar": null
    });

    let user: User = serde_json::from_value(api_json).unwrap();
    assert_eq!(user.email, Some("test@example.com".to_string()));
}

#[test]
fn task_roundtrip() {
    let task = Task {
        id: "5678".to_string(),
        task_list_id: "9999".to_string(),
        name: "Write tests".to_string(),
        is_completed: false,
        position: 65536.0,
        linked_card_id: None,
        assignee_user_id: None,
        created_at: "2026-04-14T12:00:00Z".to_string(),
        updated_at: None,
    };

    let json = serde_json::to_value(&task).unwrap();
    assert_eq!(json["taskListId"], "9999");
    assert_eq!(json["isCompleted"], false);
    let deserialized: Task = serde_json::from_value(json).unwrap();
    assert_eq!(deserialized, task);
}

#[test]
fn task_deserialize_from_planka_api() {
    let api_json = serde_json::json!({
        "id": "1754390959405139543",
        "createdAt": "2026-04-16T09:21:39.864Z",
        "updatedAt": null,
        "position": 65536,
        "name": "test task",
        "isCompleted": false,
        "taskListId": "1754390875418396246",
        "linkedCardId": null,
        "assigneeUserId": null
    });

    let task: Task = serde_json::from_value(api_json).unwrap();
    assert_eq!(task.id, "1754390959405139543");
    assert_eq!(task.task_list_id, "1754390875418396246");
    assert!(!task.is_completed);
}

#[test]
fn task_list_roundtrip() {
    let tl = TaskList {
        id: "9999".to_string(),
        card_id: "1234".to_string(),
        name: "Checklist".to_string(),
        position: 65536.0,
        created_at: "2026-04-14T12:00:00Z".to_string(),
        updated_at: None,
    };

    let json = serde_json::to_value(&tl).unwrap();
    assert_eq!(json["cardId"], "1234");
    let deserialized: TaskList = serde_json::from_value(json).unwrap();
    assert_eq!(deserialized, tl);
}

#[test]
fn envelope_serialization() {
    let envelope = Envelope {
        success: true,
        data: vec!["a", "b"],
        meta: Some(Meta { count: 2 }),
    };

    let json = serde_json::to_value(&envelope).unwrap();
    assert_eq!(json["success"], true);
    assert_eq!(json["data"], serde_json::json!(["a", "b"]));
    assert_eq!(json["meta"]["count"], 2);
}

#[test]
fn envelope_no_meta() {
    let envelope: Envelope<&str> = Envelope {
        success: true,
        data: "hello",
        meta: None,
    };

    let json = serde_json::to_value(&envelope).unwrap();
    assert!(
        json.get("meta").is_none(),
        "meta should be omitted when None"
    );
}

#[test]
fn board_membership_roundtrip() {
    let membership = BoardMembership {
        id: "900".to_string(),
        board_id: "456".to_string(),
        user_id: "88".to_string(),
        role: Some("editor".to_string()),
        can_comment: None,
        project_id: Some("123".to_string()),
        created_at: "2026-04-14T12:00:00Z".to_string(),
        updated_at: None,
    };

    let json = serde_json::to_value(&membership).unwrap();
    assert_eq!(json["boardId"], "456");
    assert_eq!(json["userId"], "88");
    assert_eq!(json["projectId"], "123");
    let deserialized: BoardMembership = serde_json::from_value(json).unwrap();
    assert_eq!(deserialized, membership);
}

#[test]
fn project_manager_roundtrip() {
    let pm = ProjectManager {
        id: "901".to_string(),
        project_id: "123".to_string(),
        user_id: "88".to_string(),
        created_at: "2026-04-14T12:00:00Z".to_string(),
        updated_at: None,
    };

    let json = serde_json::to_value(&pm).unwrap();
    assert_eq!(json["projectId"], "123");
    let deserialized: ProjectManager = serde_json::from_value(json).unwrap();
    assert_eq!(deserialized, pm);
}

#[test]
fn attachment_deserialize_from_planka_api() {
    let api_json = serde_json::json!({
        "id": "1754402698012132966",
        "createdAt": "2026-04-16T09:44:59.216Z",
        "updatedAt": null,
        "type": "file",
        "data": {
            "size": 235,
            "image": null,
            "encoding": "utf8",
            "mimeType": null,
            "url": "http://example.com/attachments/123/download/test.txt",
            "thumbnailUrls": null
        },
        "name": "test.txt",
        "cardId": "1753741395203458584",
        "creatorUserId": "1750688362236216321"
    });

    let att: Attachment = serde_json::from_value(api_json).unwrap();
    assert_eq!(att.id, "1754402698012132966");
    assert_eq!(att.name, "test.txt");
    assert_eq!(att.card_id, "1753741395203458584");
    let data = att.data.unwrap();
    assert_eq!(data.size, Some(235));
    assert!(data.url.is_some());
}

#[test]
fn comment_roundtrip() {
    let comment = Comment {
        id: "777".to_string(),
        card_id: "1234".to_string(),
        user_id: "88".to_string(),
        text: "Looks good!".to_string(),
        created_at: "2026-04-14T12:00:00Z".to_string(),
        updated_at: None,
    };

    let json = serde_json::to_value(&comment).unwrap();
    assert_eq!(json["cardId"], "1234");
    assert_eq!(json["text"], "Looks good!");
    let deserialized: Comment = serde_json::from_value(json).unwrap();
    assert_eq!(deserialized, comment);
}

#[test]
fn comment_trimmed_columns_use_wire_field_names() {
    let fields: Vec<&str> = Comment::trimmed_columns().iter().map(|(f, _)| *f).collect();
    assert_eq!(fields, vec!["id", "userId", "text", "createdAt"]);
}

#[test]
fn card_label_roundtrip() {
    let cl = CardLabel {
        id: "555".to_string(),
        card_id: "1234".to_string(),
        label_id: "111".to_string(),
        created_at: "2026-04-14T12:00:00Z".to_string(),
    };

    let json = serde_json::to_value(&cl).unwrap();
    assert_eq!(json["cardId"], "1234");
    assert_eq!(json["labelId"], "111");
    let deserialized: CardLabel = serde_json::from_value(json).unwrap();
    assert_eq!(deserialized, cl);
}

#[test]
fn label_roundtrip() {
    let label = Label {
        id: "111".to_string(),
        board_id: "456".to_string(),
        name: Some("urgent".to_string()),
        color: "berry-red".to_string(),
        position: 65536.0,
        created_at: "2026-04-14T12:00:00Z".to_string(),
        updated_at: None,
    };

    let json = serde_json::to_value(&label).unwrap();
    assert_eq!(json["boardId"], "456");
    assert_eq!(json["color"], "berry-red");
    let deserialized: Label = serde_json::from_value(json).unwrap();
    assert_eq!(deserialized, label);
}

#[test]
fn card_trimmed_columns_match_wire_format() {
    let columns = Card::trimmed_columns();
    let fields: Vec<&str> = columns.iter().map(|(f, _)| *f).collect();
    assert_eq!(fields, vec!["id", "name", "listId", "position", "isClosed"]);
    let labels: Vec<&str> = columns.iter().map(|(_, l)| *l).collect();
    assert_eq!(labels, vec!["ID", "Name", "List", "Position", "Closed"]);
}

#[test]
fn project_trimmed_columns_match_wire_format() {
    let fields: Vec<&str> = Project::trimmed_columns().iter().map(|(f, _)| *f).collect();
    assert_eq!(fields, vec!["id", "name"]);
}

/// Every Tabular field name must appear in the struct's camelCase serde
/// representation — otherwise trimmed output would produce phantom keys
/// or silently project to nothing.
#[test]
#[allow(clippy::too_many_lines)]
fn tabular_fields_exist_in_serde_representation() {
    fn check<T: serde::Serialize + Tabular + ?Sized>(item: &T, type_name: &str) {
        let value = serde_json::to_value(item).unwrap();
        let object = value.as_object().expect("serializes to object");
        for (field, _label) in T::trimmed_columns() {
            assert!(
                object.contains_key(*field),
                "Tabular field {field:?} missing from serialized {type_name}"
            );
        }
    }

    check(
        &Project {
            id: "1".into(),
            name: "p".into(),
            description: None,
            created_at: "t".into(),
            updated_at: None,
        },
        "Project",
    );
    check(
        &Board {
            id: "1".into(),
            project_id: "2".into(),
            name: "b".into(),
            position: 1.0,
            created_at: "t".into(),
            updated_at: None,
        },
        "Board",
    );
    check(
        &List {
            id: "1".into(),
            board_id: "2".into(),
            name: "l".into(),
            position: 1.0,
            color: None,
            created_at: "t".into(),
            updated_at: None,
        },
        "List",
    );
    check(
        &Card {
            id: "1".into(),
            list_id: "2".into(),
            board_id: "3".into(),
            name: "c".into(),
            description: None,
            position: 1.0,
            due_date: None,
            is_due_completed: None,
            is_closed: false,
            is_subscribed: false,
            creator_user_id: None,
            created_at: "t".into(),
            updated_at: None,
        },
        "Card",
    );
    check(
        &Task {
            id: "1".into(),
            task_list_id: "2".into(),
            name: "t".into(),
            is_completed: false,
            position: 1.0,
            linked_card_id: None,
            assignee_user_id: None,
            created_at: "t".into(),
            updated_at: None,
        },
        "Task",
    );
    check(
        &Comment {
            id: "1".into(),
            card_id: "2".into(),
            user_id: "3".into(),
            text: "hi".into(),
            created_at: "t".into(),
            updated_at: None,
        },
        "Comment",
    );
    check(
        &Label {
            id: "1".into(),
            board_id: "2".into(),
            name: None,
            color: "red".into(),
            position: 1.0,
            created_at: "t".into(),
            updated_at: None,
        },
        "Label",
    );
    check(
        &User {
            id: "1".into(),
            name: "u".into(),
            username: None,
            email: None,
            role: "editor".into(),
            is_deactivated: false,
            organization: None,
            phone: None,
            created_at: "t".into(),
            updated_at: None,
        },
        "User",
    );
    check(
        &Attachment {
            id: "1".into(),
            card_id: "2".into(),
            name: "a".into(),
            data: None,
            creator_user_id: None,
            created_at: "t".into(),
            updated_at: None,
        },
        "Attachment",
    );
    check(
        &BoardMembership {
            id: "1".into(),
            board_id: "2".into(),
            user_id: "3".into(),
            role: None,
            can_comment: None,
            project_id: None,
            created_at: "t".into(),
            updated_at: None,
        },
        "BoardMembership",
    );
    check(
        &ProjectManager {
            id: "1".into(),
            project_id: "2".into(),
            user_id: "3".into(),
            created_at: "t".into(),
            updated_at: None,
        },
        "ProjectManager",
    );
    check(
        &CardMembership {
            id: "1".into(),
            card_id: "2".into(),
            user_id: "3".into(),
            created_at: "t".into(),
            updated_at: None,
        },
        "CardMembership",
    );
    check(
        &CardLabel {
            id: "1".into(),
            card_id: "2".into(),
            label_id: "3".into(),
            created_at: "t".into(),
        },
        "CardLabel",
    );
}

#[test]
fn create_card_serializes_with_type() {
    let params = CreateCard {
        list_id: "789".to_string(),
        name: "Fix auth".to_string(),
        description: None,
        card_type: "project".to_string(),
        position: 65536.0,
    };

    let json = serde_json::to_value(&params).unwrap();
    assert_eq!(json["type"], "project");
    assert_eq!(json["listId"], "789");
    assert!(json.get("description").is_none());
}

#[test]
fn create_board_serializes_with_type() {
    let params = CreateBoard {
        project_id: "123".to_string(),
        name: "Sprint".to_string(),
        board_type: "kanban".to_string(),
        position: 65536.0,
    };

    let json = serde_json::to_value(&params).unwrap();
    assert_eq!(json["type"], "kanban");
    assert_eq!(json["projectId"], "123");
}

#[test]
fn create_list_serializes_with_type() {
    let params = CreateList {
        board_id: "456".to_string(),
        name: "Doing".to_string(),
        list_type: "active".to_string(),
        position: 65536.0,
    };

    let json = serde_json::to_value(&params).unwrap();
    assert_eq!(json["type"], "active");
    assert_eq!(json["boardId"], "456");
}
