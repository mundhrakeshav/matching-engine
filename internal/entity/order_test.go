package entity

import "testing"

func validOrder() Order {
	return Order{RestingOrder: RestingOrder{ID: 1, OriginalQty: 10, OpenQty: 10}, LimitPrice: 100, OrderKind: OrderKindLimit, OrderSide: OrderSideBuy}
}

func TestOrderValidateRequiresValidOpenQuantity(t *testing.T) {
	for name, order := range map[string]Order{
		"zero":                  func() Order { o := validOrder(); o.OpenQty = 0; return o }(),
		"greater than original": func() Order { o := validOrder(); o.OpenQty = 11; return o }(),
	} {
		t.Run(name, func(t *testing.T) {
			if err := order.Validate(); err == nil {
				t.Fatal("Validate() = nil, want error")
			}
		})
	}
}

func TestMarketOrderDoesNotRequireLimitPrice(t *testing.T) {
	order := validOrder()
	order.OrderKind = OrderKindMarket
	order.LimitPrice = 0
	if err := order.Validate(); err != nil {
		t.Fatalf("Validate() = %v, want nil", err)
	}
}
