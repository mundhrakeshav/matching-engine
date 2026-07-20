package entity

import (
	"errors"
	"time"
)

const (
	OrderSideUnknown OrderSideType = iota
	OrderSideBuy
	OrderSideSell
)

const (
	OrderKindUnknown OrderKindType = iota
	OrderKindMarket
	OrderKindLimit
)

type Order struct {
	RestingOrder
	LimitPrice   Price         `json:"limitPrice"`
	Expiry       time.Time     `json:"expiry"`
	OrderKind    OrderKindType `json:"kind"`
	OrderSide    OrderSideType `json:"side"`
	AllowPartial bool          `json:"allowPartial"`
}

type OrderNode struct {
	RestingOrder RestingOrder
	Next         NodeID
	Prev         NodeID
}

func (k OrderKindType) validate() error {
	if k != OrderKindMarket && k != OrderKindLimit {
		return errors.New("invalid order kind")
	}
	return nil
}

func (s OrderSideType) validate() error {
	if s != OrderSideBuy && s != OrderSideSell {
		return errors.New("invalid order side")
	}
	return nil
}

func (o *Order) Validate() error {
	if err := o.OrderKind.validate(); err != nil {
		return err
	}
	if err := o.OrderSide.validate(); err != nil {
		return err
	}

	if o.OriginalQty <= 0 {
		return errors.New("quantity must be positive")
	}

	if o.LimitPrice <= 0 {
		return errors.New("price must be positive")
	}
	return nil
}

// Crosses reports whether the order's limit price is aggressive enough to
// match against the opposite side's best price.
func (o *Order) Crosses(price Price) bool {
	if o.OrderSide == OrderSideBuy {
		return o.LimitPrice >= price
	}
	return o.LimitPrice <= price
}
