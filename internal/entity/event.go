package entity

const (
	OperationUnknown OperationType = iota
	OperationNewOrder
	OperationModifyOrder
	OperationCancelOrder
)

type OrderBookEvent struct {
	Order Order         `json:"order"`
	Op    OperationType `json:"op"`
}
