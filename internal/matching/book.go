package matching

import (
	"context"
	"fmt"
	"ob/internal/entity"
	"ob/internal/matching/pool"
	"ob/pkg/log"

	"github.com/google/btree"
)

var (
	errFailedToGetNode = func(node entity.NodeID, err error) error {
		return fmt.Errorf("failed to get node %d %w", node, err)
	}

	errInvariantViolation = func(msg string) error {
		return fmt.Errorf("invariant violation: %s", msg)
	}
)

type Book struct {
	orderpool pool.Pool[entity.OrderNode]
	asks      *btree.BTreeG[*entity.PriceLevel]
	bids      *btree.BTreeG[*entity.PriceLevel]
	locations map[entity.OrderID]entity.NodeID
}

func NewBook(poolSize uint64) *Book {
	return &Book{
		asks: btree.NewG(2, func(a, b *entity.PriceLevel) bool {
			return a.Less(b)
		}),
		bids: btree.NewG(2, func(a, b *entity.PriceLevel) bool {
			return b.Less(a)
		}),
		orderpool: pool.New[entity.OrderNode](poolSize),
		locations: make(map[entity.OrderID]entity.NodeID),
	}
}

func (b *Book) SubmitOrder(ctx context.Context, o *entity.Order) error {
	err := o.Validate()
	if err != nil {
		return err
	}

	switch o.OrderKind {
	case entity.OrderKindMarket:
		return b.submitMarket(ctx, o)

	case entity.OrderKindLimit:
		return b.submitLimit(ctx, o)
	}

	return nil
}

func (b *Book) submitMarket(ctx context.Context, o *entity.Order) error {

	return nil
}

func (b *Book) submitLimit(ctx context.Context, o *entity.Order) error {
	logger := log.GetLogger(ctx)
	logger.Info("Got Limit Order", log.Any("order", o))

	return nil
}

func (b *Book) oppositeBookFor(side entity.OrderSideType) *btree.BTreeG[*entity.PriceLevel] {
	if side == entity.OrderSideBuy {
		return b.asks
	}
	return b.bids
}

func (b *Book) match(ctx context.Context, o *entity.Order) error {
	book := b.oppositeBookFor(o.OrderSide)

	for o.OpenQty > 0 {
		level, ok := book.Min()
		if !ok {
			break
		}
		if o.OrderKind == entity.OrderKindLimit && !o.Crosses(level.Price()) {
			break
		}
		if err := b.matchHeadAtLevel(book, level, o); err != nil {
			return err
		}
	}

	return nil
}

func (b *Book) matchHeadAtLevel(
	book *btree.BTreeG[*entity.PriceLevel],
	level *entity.PriceLevel,
	o *entity.Order,
) error {
	node, err := b.orderpool.Get(level.Head)
	if err != nil {
		return errFailedToGetNode(level.Head, err)
	}

	fill := min(o.OpenQty, node.RestingOrder.OpenQty)
	if level.Qty < fill {
		return errInvariantViolation(fmt.Sprintf("level qty %d is less than fill %d", level.Qty, fill))
	}

	o.OpenQty -= fill
	node.RestingOrder.OpenQty -= fill
	level.Qty -= fill

	if node.RestingOrder.OpenQty > 0 {
		return nil
	}
	return b.removeHead(book, level, node)
}

func (b *Book) removeHead(
	book *btree.BTreeG[*entity.PriceLevel],
	level *entity.PriceLevel,
	node *entity.OrderNode,
) error {
	nodeID := level.Head
	orderID := node.RestingOrder.ID

	if err := b.advanceLevelHead(level, node); err != nil {
		return err
	}
	if err := b.orderpool.Release(nodeID); err != nil {
		return fmt.Errorf("failed to release order node %d: %w", nodeID, err)
	}
	delete(b.locations, orderID)

	if level.Head == entity.NoneU64 {
		book.Delete(level)
	}
	return nil
}

func (b *Book) advanceLevelHead(level *entity.PriceLevel, node *entity.OrderNode) error {
	nextIdx := node.Next
	if nextIdx == entity.NoneU64 {
		level.Head = entity.NoneU64
		level.Tail = entity.NoneU64
	} else {
		next, err := b.orderpool.Get(nextIdx)
		if err != nil {
			return errFailedToGetNode(nextIdx, err)
		}
		next.Prev = entity.NoneU64
		level.Head = nextIdx
	}
	level.Count--
	return nil
}
