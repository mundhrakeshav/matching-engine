package matching_test

import (
	"context"
	"ob/internal/entity"
	"ob/internal/matching"
	"testing"
	"time"
)

func TestSubmitOrder(t *testing.T) {
	ctx := context.Background()
	book := matching.NewBook(1025)
	engine := matching.NewEngine(book)
	go engine.Run(ctx)
	err := engine.SubmitOrder(&entity.Order{
		RestingOrder: entity.RestingOrder{
			ID:          1,
			OriginalQty: 100,
			UserID:      1,
			OpenQty:     100,
			AcceptedSeq: 1,
		},
		LimitPrice: 10000,
		OrderKind:  entity.OrderKindLimit,
		OrderSide:  entity.OrderSideBuy,
	})
	if err != nil {
		t.Fatalf("failed to submit order: %v", err)
	}
	time.Sleep(1 * time.Second)
}
