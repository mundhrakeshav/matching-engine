package entity

import "encoding/json"

type Action string

const (
	ActionNew    Action = "new"
	ActionModify Action = "modify"
	ActionCancel Action = "cancel"
)

type OrderBookEvent struct {
	Order  json.RawMessage
	Action Action
}
