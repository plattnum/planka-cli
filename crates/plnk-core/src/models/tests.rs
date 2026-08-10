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
    check(
        &BaseCustomFieldGroup {
            id: "1".into(),
            project_id: "2".into(),
            name: "Documentation".into(),
            created_at: "t".into(),
            updated_at: None,
        },
        "BaseCustomFieldGroup",
    );
    check(
        &CustomFieldGroup {
            id: "1".into(),
            name: None,
            board_id: None,
            card_id: Some("2".into()),
            base_custom_field_group_id: Some("3".into()),
            position: 65536.0,
            created_at: "t".into(),
            updated_at: None,
        },
        "CustomFieldGroup",
    );
    check(
        &CustomField {
            id: "1".into(),
            name: "Specification".into(),
            position: 65536.0,
            show_on_front_of_card: true,
            custom_field_group_id: Some("2".into()),
            base_custom_field_group_id: None,
            created_at: "t".into(),
            updated_at: None,
        },
        "CustomField",
    );
    check(
        &CustomFieldValue {
            id: "1".into(),
            card_id: "2".into(),
            custom_field_group_id: "3".into(),
            custom_field_id: "4".into(),
            content: "specs/x.html".into(),
            created_at: "t".into(),
            updated_at: None,
        },
        "CustomFieldValue",
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

// --- custom fields -------------------------------------------------------
//
// Payloads below are captured verbatim from a live Planka server (2026-08-10).

#[test]
fn base_custom_field_group_deserialize_from_planka_api() {
    let api_json = serde_json::json!({
        "id": "1838314588785870118",
        "createdAt": "2026-08-10T04:22:56.098Z",
        "updatedAt": null,
        "name": "Documentation",
        "projectId": "1838286221441238832"
    });

    let group: BaseCustomFieldGroup = serde_json::from_value(api_json).unwrap();
    assert_eq!(group.id, "1838314588785870118");
    assert_eq!(group.name, "Documentation");
    assert_eq!(group.project_id, "1838286221441238832");
    assert!(group.updated_at.is_none());
}

/// A base group's create response omits `position` entirely even though the
/// create request accepts one — the model must not require it.
#[test]
fn base_custom_field_group_deserializes_without_position() {
    let api_json = serde_json::json!({
        "id": "1",
        "createdAt": "t",
        "updatedAt": null,
        "name": "Docs",
        "projectId": "2"
    });

    let group: BaseCustomFieldGroup = serde_json::from_value(api_json).unwrap();
    let round_tripped = serde_json::to_value(&group).unwrap();
    assert!(
        round_tripped.get("position").is_none(),
        "base groups carry no position"
    );
}

/// The load-bearing case: a card group adopted from a base group has a null
/// `name`, with the display name living on the base group.
#[test]
fn custom_field_group_adopted_from_base_has_null_name() {
    let api_json = serde_json::json!({
        "id": "1838315093998175530",
        "createdAt": "2026-08-10T04:23:56.324Z",
        "updatedAt": null,
        "position": 81920.0,
        "name": null,
        "boardId": null,
        "cardId": "1838300473224856753",
        "baseCustomFieldGroupId": "1838314588785870118"
    });

    let group: CustomFieldGroup = serde_json::from_value(api_json).unwrap();
    assert!(group.name.is_none(), "adopted groups carry a null name");
    assert_eq!(
        group.base_custom_field_group_id,
        Some("1838314588785870118".to_string())
    );
    assert_eq!(group.card_id, Some("1838300473224856753".to_string()));
    assert!(group.board_id.is_none());
}

#[test]
fn custom_field_group_local_to_card_has_name_and_no_base() {
    let api_json = serde_json::json!({
        "id": "1838305759222301878",
        "createdAt": "2026-08-10T04:05:23.532Z",
        "updatedAt": "2026-08-10T04:09:26.759Z",
        "position": 65536.0,
        "name": "Documentation",
        "boardId": null,
        "cardId": "1838300473224856753",
        "baseCustomFieldGroupId": null
    });

    let group: CustomFieldGroup = serde_json::from_value(api_json).unwrap();
    assert_eq!(group.name, Some("Documentation".to_string()));
    assert!(group.base_custom_field_group_id.is_none());
}

#[test]
fn custom_field_belonging_to_base_group_deserializes() {
    let api_json = serde_json::json!({
        "id": "1838314715311244583",
        "createdAt": "2026-08-10T04:23:11.181Z",
        "updatedAt": null,
        "position": 65536.0,
        "name": "Specification",
        "showOnFrontOfCard": true,
        "baseCustomFieldGroupId": "1838314588785870118",
        "customFieldGroupId": null
    });

    let field: CustomField = serde_json::from_value(api_json).unwrap();
    assert_eq!(field.name, "Specification");
    assert!(field.show_on_front_of_card);
    assert!(field.custom_field_group_id.is_none());
    assert_eq!(
        field.base_custom_field_group_id,
        Some("1838314588785870118".to_string())
    );
}

#[test]
fn custom_field_belonging_to_card_group_deserializes() {
    let api_json = serde_json::json!({
        "id": "1838307535082226873",
        "createdAt": "2026-08-10T04:08:55.229Z",
        "updatedAt": null,
        "position": 65536.0,
        "name": "Specification",
        "showOnFrontOfCard": false,
        "baseCustomFieldGroupId": null,
        "customFieldGroupId": "1838305759222301878"
    });

    let field: CustomField = serde_json::from_value(api_json).unwrap();
    assert!(!field.show_on_front_of_card);
    assert_eq!(
        field.custom_field_group_id,
        Some("1838305759222301878".to_string())
    );
    assert!(field.base_custom_field_group_id.is_none());
}

#[test]
fn custom_field_value_roundtrip_camel_case() {
    let value = CustomFieldValue {
        id: "1".to_string(),
        card_id: "2".to_string(),
        custom_field_group_id: "3".to_string(),
        custom_field_id: "4".to_string(),
        content: "specs/2026-08-06-codex-design.html".to_string(),
        created_at: "2026-08-10T04:30:00.000Z".to_string(),
        updated_at: None,
    };

    let json = serde_json::to_value(&value).unwrap();
    assert_eq!(json["cardId"], "2");
    assert_eq!(json["customFieldGroupId"], "3");
    assert_eq!(json["customFieldId"], "4");
    assert_eq!(json["content"], "specs/2026-08-06-codex-design.html");
    assert!(json.get("card_id").is_none(), "should use camelCase");

    let deserialized: CustomFieldValue = serde_json::from_value(json).unwrap();
    assert_eq!(deserialized, value);
}

#[test]
fn custom_field_trimmed_columns_match_wire_format() {
    let fields: Vec<&str> = CustomField::trimmed_columns()
        .iter()
        .map(|(f, _)| *f)
        .collect();
    assert_eq!(
        fields,
        vec!["id", "name", "showOnFrontOfCard", "position"],
        "trimmed columns must use wire spelling, never display labels"
    );

    let group_fields: Vec<&str> = CustomFieldGroup::trimmed_columns()
        .iter()
        .map(|(f, _)| *f)
        .collect();
    assert_eq!(
        group_fields,
        vec!["id", "name", "cardId", "boardId", "position"]
    );

    let value_fields: Vec<&str> = CustomFieldValue::trimmed_columns()
        .iter()
        .map(|(f, _)| *f)
        .collect();
    assert_eq!(value_fields, vec!["id", "customFieldId", "content"]);

    let base_fields: Vec<&str> = BaseCustomFieldGroup::trimmed_columns()
        .iter()
        .map(|(f, _)| *f)
        .collect();
    assert_eq!(base_fields, vec!["id", "name", "projectId"]);
}

#[test]
fn update_custom_field_omits_unset_fields() {
    let params = UpdateCustomField {
        name: Some("Spec".to_string()),
        show_on_front_of_card: None,
    };
    let json = serde_json::to_value(&params).unwrap();
    assert_eq!(json["name"], "Spec");
    assert!(
        json.get("showOnFrontOfCard").is_none(),
        "unset must be omitted, not sent as null"
    );
}

/// "Set to false" and "leave unchanged" must stay distinguishable — an
/// explicit `false` has to reach the wire.
#[test]
fn update_custom_field_sends_explicit_false() {
    let params = UpdateCustomField {
        name: None,
        show_on_front_of_card: Some(false),
    };
    let json = serde_json::to_value(&params).unwrap();
    assert_eq!(json["showOnFrontOfCard"], false);
    assert!(json.get("name").is_none());
}

#[test]
fn update_custom_field_group_omits_unset_name() {
    let params = UpdateCustomFieldGroup { name: None };
    let json = serde_json::to_value(&params).unwrap();
    assert_eq!(json.as_object().unwrap().len(), 0);
}
