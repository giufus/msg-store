## in memory storage

### gRPC call
```bash
grpcurl -plaintext -import-path ./proto -proto store.proto -d '{"tenant": "3bd1c699", "key": "K-h53dk-B"}' '[::1]:8080' messages.Storage.Process
```

