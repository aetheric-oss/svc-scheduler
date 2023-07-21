![Arrow Banner](https://github.com/Arrow-air/tf-github/raw/main/src/templates/doc-banner-services.png)

# Router Library - Software Design Document (SDD)

### Metadata

| Attribute     | Description                                                       |
| ------------- |-------------------------------------------------------------------|
| Maintainer(s) | [Services Team](https://github.com/orgs/Arrow-air/teams/services) |
| Stuckee       | [GoodluckH](https://github.com/GoodluckH)                         |
| Status        | Development                                                       |

## Overview

This document details the software implementation of the Router library.

This library provides a set of functions to help with the routing needs of the client application.

Attribute | Description
--- | ---
Status | Draft

## Module Attributes

Attribute | Applies | Explanation
--- | --- | ---
Safety Critical | No | Does not have direct impact on how aircraft is controlled.
Realtime | Yes | Expect production-grade usage to involve realtime parameters to compute the weight between two nodes. 

## Logic 

The router builds a graph of nodes and edges. The nodes represent taking-off and
landing points like vertiports, and the edges are the routes between points. The
router can also find the shortest path between two nodes, or lack thereof.

Under the hood, the graph is largely handled by the
[petgraph](https://docs.rs/petgraph/latest/petgraph/) library. For safety
purposes, the graph itself is private. Instead, the router provides a set of
public functions that implement the `Router` struct, customized to Arrow's
business logics, to interact with the graph.

### Structure
The Router itself is represented as a `struct` wrapping three data structures: 

* **Stable Graph**: this is a petgraph implementation that allows for stable
  deletions. Learn more
  [here](https://docs.rs/petgraph/latest/petgraph/stable_graph/struct.StableGraph.html).
  This is the core of the router engine because all other functionalities are
  built on top of the graph.

* **Hash Map of Node Indices**: a hash map with `Node` as keys and
  [`NodeIndex`](https://docs.rs/petgraph/latest/petgraph/graph/struct.NodeIndex.html)
  as values. This data structure is needed because petgraph uses `NodeIndex` for
  accessing the actual nodes in a graph, and users might not have `NodeIndex` at
  their disposal. The hash map allows for quick lookup for the `NodeIndex`
  associated with a `Node`.

* **Vector of Edges**: Storing a vector of edges allows users to quickly
  retrieve all edges in the graph without having to perform expensive on-the-fly
  queries to generate edges. We assume that edges are only occasionally get
  added or removed, but client-side applications may frequently ask for edges of
  a graph. Therefore, although extra spaces are needed to allocate this vector
  of edges, the savings on runtime over time outweigh the space trade-off.

### Initialization

The `Router` struct has a `new()` method for initialization, and it expects the following:

* `nodes`: a reference to a vector of objects that implement the `AsNode` trait.
  These objects could be a vertiport, a floating pad, a rooftop etc.
* `constraint`: a float-32 value that restricts the graph from connecting nodes.
  A constraint could be the distance between any two nodes. For example, a
  constraint of 100 tells the `Router` to not connect nodes that are 100 units
  apart.
* `constraint_function`: a function that returns a float-32 value that is used
  to compare against `constraint`.
* `cost_function`: edges have weights. The `cost_function` is used to compute
  the weight between any two nodes.

The `new` initialization method will return a `Router` struct that includes a
constructed graph. Users can then call other functions to get nodes and edges
or find the shortest paths.

## Shortest Path Algorithms
Petgraph has a built-in function for finding the shortest path between two
points. And it supports
[Dijkstra](https://en.wikipedia.org/wiki/Dijkstra%27s_algorithm) and
[A-Star](https://en.wikipedia.org/wiki/A*_search_algorithm), two prominent
path-finding algorithms. 

The two algorithms are nearly identical in petgraph's implementation, the only
difference is that A-Star accepts an extra heuristic function for a more guided
path-finding process.

We are still testing the efficiency and accuracy of these path-finding
algorithms to decide on which one to use under what circumstances. 

## Tests

We thoroughly test every function in this library to ensure correct and fast routing.
### Unit Tests

At 100% coverage.

*Note: the unit test for generating random nodes within a certain radius of a
point can fail sometimes. This is because `gen_around_location` sometimes
returns incorrect values. However, this function is only used for testing
purposes.*
