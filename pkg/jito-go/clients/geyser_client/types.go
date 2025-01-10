package geyser_client

import (
	"context"

	"github.com/newruscult/xo-sniper/pkg/jito-go/proto"
	"google.golang.org/grpc"
)

type Client struct {
	GrpcConn *grpc.ClientConn
	Ctx      context.Context

	Geyser proto.GeyserClient

	ErrChan <-chan error
}
