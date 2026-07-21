package matching

import (
	"ob/internal/entity"
	"testing"
)

func addTestNode(t *testing.T, b *Book, order entity.RestingOrder, next entity.NodeID) entity.NodeID {
	t.Helper()
	idx, err := b.orderpool.Allocate(entity.OrderNode{RestingOrder: order, Next: next, Prev: entity.NoneU64})
	if err != nil {
		t.Fatalf("allocate node: %v", err)
	}
	node, err := b.orderpool.Get(idx)
	if err != nil {
		t.Fatalf("get node: %v", err)
	}
	node.ID = idx
	node.Price = 100
	return idx
}

func TestMatchFlowMatchesHealthyBookAndReleasesFilledHead(t *testing.T) {
	b := NewBook(2)
	level := entity.NewPriceLevel(100)
	second := addTestNode(t, b, entity.RestingOrder{ID: 2, OpenQty: 5}, entity.NoneU64)
	head := addTestNode(t, b, entity.RestingOrder{ID: 1, OpenQty: 5}, second)
	secondNode, _ := b.orderpool.Get(second)
	secondNode.Prev = head
	level.Head, level.Tail, level.Count, level.Qty = head, second, 2, 10
	b.bids.ReplaceOrInsert(level)
	taker := &entity.Order{RestingOrder: entity.RestingOrder{ID: 9, OpenQty: 5}, OrderSide: entity.OrderSideSell, OrderKind: entity.OrderKindMarket}

	headNode, err := b.orderpool.Get(head)
	if err != nil {
		t.Fatalf("get head: %v", err)
	}
	match := b.compute(taker, headNode)
	if err := b.validate(headNode, match, level, b.bids); err != nil {
		t.Fatalf("validate: %v", err)
	}
	if err := b.apply(taker, match, headNode, level, b.bids); err != nil {
		t.Fatalf("apply: %v", err)
	}
	if match.Trade.Qty != 5 || match.Trade.MakerOrderID != 1 {
		t.Fatalf("unexpected trade: %#v", match.Trade)
	}
	if level.Head != second || level.Tail != second || level.Count != 1 || level.Qty != 5 {
		t.Fatalf("unexpected level after match: %#v", level)
	}
	if _, err := b.orderpool.Get(head); err == nil {
		t.Fatal("filled head was not released")
	}
}

func TestMatchAllowsHealthyBook(t *testing.T) {
	b := NewBook(1)
	level := entity.NewPriceLevel(100)
	head := addTestNode(t, b, entity.RestingOrder{ID: 1, OpenQty: 5}, entity.NoneU64)
	level.Head, level.Tail, level.Count, level.Qty = head, head, 1, 5
	b.asks.ReplaceOrInsert(level)
	taker := &entity.Order{RestingOrder: entity.RestingOrder{ID: 9, OpenQty: 5}, OrderSide: entity.OrderSideBuy, OrderKind: entity.OrderKindMarket}

	if err := b.match(nil, taker); err != nil {
		t.Fatalf("match on healthy book: %v", err)
	}
	if taker.OpenQty != 0 {
		t.Fatalf("taker open quantity = %d, want 0", taker.OpenQty)
	}
}

func TestSubmitOrderRejectsNil(t *testing.T) {
	if err := NewBook(1).SubmitOrder(nil, nil); err == nil {
		t.Fatal("SubmitOrder(nil) = nil, want error")
	}
}

func TestValidateRejectsMalformedLevel(t *testing.T) {
	b := NewBook(1)
	level := entity.NewPriceLevel(100)
	head := addTestNode(t, b, entity.RestingOrder{ID: 1, OpenQty: 1}, entity.NoneU64)
	level.Head, level.Tail, level.Count, level.Qty = head, head, 0, 1
	b.bids.ReplaceOrInsert(level)
	taker := &entity.Order{RestingOrder: entity.RestingOrder{ID: 9, OpenQty: 1}, OrderSide: entity.OrderSideSell, OrderKind: entity.OrderKindMarket}

	headNode, err := b.orderpool.Get(head)
	if err != nil {
		t.Fatalf("get head: %v", err)
	}
	match := b.compute(taker, headNode)
	if err := b.validate(headNode, match, level, b.bids); err == nil {
		t.Fatal("validate() = nil, want error")
	}
}

func TestAdvanceLevelDoesNotMutateUnderflowingLevel(t *testing.T) {
	b := NewBook(1)
	level := entity.NewPriceLevel(100)
	level.Head, level.Tail = 7, 8
	if err := b.advanceLevel(level, entity.NoneU64, nil); err != errLevelUnderflow {
		t.Fatalf("advanceLevel() = %v, want %v", err, errLevelUnderflow)
	}
	if level.Head != 7 || level.Tail != 8 || level.Count != 0 {
		t.Fatalf("level mutated on underflow: %#v", level)
	}
}
