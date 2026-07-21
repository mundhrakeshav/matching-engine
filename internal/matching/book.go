package matching

import (
	"context"
	"errors"
	"fmt"
	"ob/internal/entity"
	"ob/internal/matching/pool"
	"ob/pkg/log"
	"time"

	"github.com/google/btree"
)

var (
	errFailedToGetNode        = errors.New("failed to get node")
	errUnhealthy              = errors.New("unhealthy book")
	errInvalidBook            = errors.New("invalid book")
	errInvalidMatch           = errors.New("invalid match result")
	errInvalidNode            = errors.New("invalid order node")
	errInvalidLevel           = errors.New("invalid level")
	errInvalidLevelChain      = errors.New("invalid level chain")
	errInvalidTradeQty        = errors.New("invalid trade quantity")
	errInvalidMatchMaker      = errors.New("match maker does not match level head")
	errInvalidMatchPrice      = errors.New("match price does not match level price")
	errInsufficientQtyAtLevel = errors.New("insufficient qty at level")
	errLevelUnderflow         = errors.New("level underflow")
)

type Book struct {
	orderpool pool.Pool[entity.OrderNode]
	asks      *btree.BTreeG[*entity.PriceLevel]
	bids      *btree.BTreeG[*entity.PriceLevel]
	locations map[entity.OrderID]entity.NodeID
	unhealthy bool
}

type matchResult struct {
	MakerDone       bool
	NodeID          entity.NodeID
	NextNodeID      entity.NodeID
	NextNode        *entity.OrderNode
	Trade           entity.Trade
	nextAcceptedSeq entity.SequenceNumber
	nextTradeSeq    entity.SequenceNumber
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
	if o == nil {
		return errors.New("order must not be nil")
	}
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
	if b.unhealthy {
		return errUnhealthy
	}
	book := b.oppositeBookFor(o.OrderSide)
	trades := make([]entity.Trade, 0)
	for o.OpenQty > 0 {
		if b.unhealthy {
			return errUnhealthy
		}
		level, ok := book.Min()
		if !ok {
			break
		}
		if o.OrderKind == entity.OrderKindLimit && !o.Crosses(level.Price()) {
			break
		}

		head, err := b.orderpool.Get(level.Head)
		if err != nil {
			return errFailedToGetNode
		}

		match := b.compute(o, head)
		if err != nil {
			return err
		}

		if err = b.validate(head, match, level, book); err != nil {
			b.unhealthy = true
			break
		}

		if err = b.apply(
			o,
			match,
			head,
			level,
			book,
		); err != nil {
			b.unhealthy = true
			return nil
		}

		trades = append(trades, match.Trade)

		// TODO
		// trade.ID
		// trade.Sequence
		// trade.InstrumentID

	}

	return nil
}

// matchHeadAtLevel matches the incoming order o against the head of the given price level.
// It returns the fill quantity and any error encountered. If the resting order at the head
// is fully consumed, the node is removed from the level and book.
func (b *Book) compute(
	o *entity.Order,
	node *entity.OrderNode,
) (mr *matchResult) {
	fill := min(o.OpenQty, node.RestingOrder.OpenQty)

	return &matchResult{
		NodeID:     node.ID,
		NextNodeID: node.Next,
		MakerDone:  node.RestingOrder.OpenQty <= fill,
		Trade: entity.Trade{
			MakerOrderID: node.RestingOrder.ID,
			TakerOrderID: o.ID,
			MakerID:      node.RestingOrder.UserID,
			TakerID:      o.UserID,
			TakerSide:    o.OrderSide,
			Qty:          fill,
			Price:        node.Price,
			CreatedAt:    time.Now(),
		},
	}
}

func (b *Book) validate(
	head *entity.OrderNode,
	match *matchResult,
	level *entity.PriceLevel,
	book *btree.BTreeG[*entity.PriceLevel],
) error {
	if b == nil || book == nil {
		return errInvalidBook
	}
	if head == nil || match == nil || level == nil {
		return errInvalidMatch
	}
	if !book.Has(level) {
		return errInvalidLevel
	}
	if level.Count == 0 {
		return errLevelUnderflow
	}
	if level.Head == entity.NoneU64 || level.Tail == entity.NoneU64 {
		return errInvalidLevelChain
	}
	if match.NodeID != level.Head || head.ID != level.Head {
		return errInvalidNode
	}
	if head.Prev != entity.NoneU64 || head.RestingOrder.OpenQty == 0 {
		return errInvalidNode
	}
	tail, err := b.orderpool.Get(level.Tail)
	if err != nil {
		return errFailedToGetNode
	}
	if tail.ID != level.Tail || tail.Next != entity.NoneU64 {
		return errInvalidLevelChain
	}
	if match.Trade.Qty == 0 || match.Trade.Qty > head.RestingOrder.OpenQty {
		return errInvalidTradeQty
	}
	if match.Trade.Qty > level.Qty {
		return errInsufficientQtyAtLevel
	}
	if match.Trade.Price != level.Price() {
		return errInvalidMatchPrice
	}
	if head.RestingOrder.ID != match.Trade.MakerOrderID {
		return errInvalidMatchMaker
	}
	if match.MakerDone != (match.Trade.Qty == head.RestingOrder.OpenQty) {
		return errInvalidMatch
	}
	match.NextNode = nil

	if level.Count == 1 {
		if level.Head != level.Tail || head.Next != entity.NoneU64 || match.NextNodeID != entity.NoneU64 {
			return errInvalidLevelChain
		}
	} else {
		if level.Head == level.Tail || head.Next == entity.NoneU64 {
			return errInvalidLevelChain
		}
		if match.NextNodeID != head.Next {
			return errInvalidMatch
		}
		next, err := b.orderpool.Get(head.Next)
		if err != nil {
			return errFailedToGetNode
		}
		if next == nil || next.Prev != level.Head {
			return errInvalidLevelChain
		}
		match.NextNode = next
	}

	return nil
}

func (b *Book) apply(
	o *entity.Order,
	match *matchResult,
	head *entity.OrderNode,
	level *entity.PriceLevel,
	book *btree.BTreeG[*entity.PriceLevel],
) error {
	head.RestingOrder.OpenQty -= match.Trade.Qty
	level.Qty -= match.Trade.Qty
	o.OpenQty -= match.Trade.Qty

	if match.MakerDone {
		if err := b.advanceLevel(level, match.NextNodeID, match.NextNode); err != nil {
			return err
		}

		if level.Head == entity.NoneU64 {
			book.Delete(level)
		}

		// Release the node from the order pool and put it back in the free list
		if err := b.orderpool.Release(head.ID); err != nil {
			return fmt.Errorf("failed to release order node %d: %w", head, err)
		}
	}

	return nil
}

// advanceLevel removes the current head from a price level and promotes the
// successor that was already loaded during validation. For a single-node
// level, nextNodeID must be NoneU64 and nextNode must be nil; the level is
// then emptied. For a non-empty level, nextNode must be non-nil and its ID
// must match nextNodeID.
func (b *Book) advanceLevel(
	level *entity.PriceLevel,
	nextNodeID entity.NodeID,
	nextNode *entity.OrderNode,
) error {
	if level.Count == 0 {
		return errLevelUnderflow
	}
	if nextNodeID == entity.NoneU64 {
		if nextNode != nil {
			return errInvalidLevelChain
		}
		level.Head = entity.NoneU64
		level.Tail = entity.NoneU64
	} else {
		if nextNode == nil || nextNode.ID != nextNodeID {
			return errInvalidLevelChain
		}
		nextNode.Prev = entity.NoneU64
		level.Head = nextNodeID
	}
	level.Count--
	return nil
}
