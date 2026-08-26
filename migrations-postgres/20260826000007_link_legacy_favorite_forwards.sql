UPDATE messages
SET favorite_id = (
    SELECT favorites.id
    FROM favorites
    WHERE favorites.user_id = messages.sender_id
      AND (CASE
          WHEN favorites.kind = 'manual' AND favorites.content = '' THEN favorites.title
          ELSE favorites.content
      END) = messages.content
      AND favorites.attachment_id IS NOT DISTINCT FROM messages.attachment_id
      AND (
          (favorites.kind = 'manual'
              AND messages.forwarded_from_sender = '我的收藏'
              AND messages.forwarded_from_room_name = '个人收藏')
          OR (favorites.kind <> 'manual'
              AND favorites.source_sender = messages.forwarded_from_sender
              AND favorites.source_room_name = messages.forwarded_from_room_name)
      )
    LIMIT 1
)
WHERE messages.favorite_id IS NULL
  AND messages.forwarded_from_sender IS NOT NULL
  AND (
      SELECT COUNT(*)
      FROM favorites
      WHERE favorites.user_id = messages.sender_id
        AND (CASE
            WHEN favorites.kind = 'manual' AND favorites.content = '' THEN favorites.title
            ELSE favorites.content
        END) = messages.content
        AND favorites.attachment_id IS NOT DISTINCT FROM messages.attachment_id
        AND (
            (favorites.kind = 'manual'
                AND messages.forwarded_from_sender = '我的收藏'
                AND messages.forwarded_from_room_name = '个人收藏')
            OR (favorites.kind <> 'manual'
                AND favorites.source_sender = messages.forwarded_from_sender
                AND favorites.source_room_name = messages.forwarded_from_room_name)
        )
  ) = 1;
