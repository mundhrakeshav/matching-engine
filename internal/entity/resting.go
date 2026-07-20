package entity

type RestingOrder struct {
	ID          OrderID
	UserID      UserID
	OriginalQty Quantity
	OpenQty     Quantity
	AcceptedSeq SequenceNumber
}
