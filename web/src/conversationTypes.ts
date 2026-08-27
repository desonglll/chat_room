export interface MessagePreview {
  message_id: string
  sender_id: string | null
  sender: string
  content: string
  attachment_file_name: string | null
  recalled: boolean
  created_at: string
}
