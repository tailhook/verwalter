import { ApolloClient } from 'apollo-client';
import { InMemoryCache } from 'apollo-cache-inmemory';
import { WebSocketLink } from "apollo-link-ws";
import { SubscriptionClient } from "subscriptions-transport-ws";
import gql from 'graphql-tag';

const GRAPHQL_ENDPOINT = "ws://"+location.host+"/v1/graphql";

const ws_client = new SubscriptionClient(GRAPHQL_ENDPOINT, {
  reconnect: true
});

const link = new WebSocketLink(ws_client);

const client = new ApolloClient({
    link,
    cache: new InMemoryCache(),
});

export var status = {peers: {}, leader: {}, fetch: {}}

export function start(render) {
    let q = client.subscribe({
      query: gql`
            subscription {
                status {
                    version
                    roles {
                        number
                        failed
                    }
                    peers {
                        number
                    }
                    leader {
                        name
                        addr
                        debugForced
                    }
                    fetch {
                        state
                    }
                    scheduleStatus
                }
            }
        `,
      variables: {}
    }).subscribe({
      next (data) {
        let time = Date.now()
        status = data.data.status
        render()
      }
    });
}
