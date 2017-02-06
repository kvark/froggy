# froggy
Component Graph System is an alternative approach to ECS. It shares a lot in common with ECS and was derived from it. CGS design expands on the original ECS concept and simplifies it to the bare minimum.

### Problem
Traditional ECS split the object semantics into components. Components are simple structures, designed to be as isolated as possible. An entity in such a system can be one of the following:
  * Just an ID. Each component knows about the entity it belongs to.
  * Just an ID. Each component manager can map it to the relevant component data.
  * A collection of component IDs.
  * An ID that can be used by the world to index into an entity array. These would contain the component IDs.

It's rather nice on paper, but then in practice I faced the harsh reality...

First, how do systems efficiently process components? If a system just works on a single component, it would optimally want to iterate the component array directly. Non-trivial systems would want to use multiple components. If the entity doesn't know about its components, iterating multiple components suddenly becomes a major performance issue.

Ok, supposing the entity does know its components. And we decided that each system processes relevant entity data by iterating through entities first. Now we have another problem - growing the number of entities slows down every system, even the ones that are not related... Thus, different entities with no intersecting subsets of components start to affect each other indirectly.

What people are doing to solve this is creating a separate list of entities per system to process. Entities are split into prototypes, or classes. This involves another big problem of synchronizing each prototype list with the main entity list. It also increases the overall complexity: we now have a component, an entity, and a prototype, not to mention all the storages and managers to handle them.

Second, in real [scenarios](http://gamedev.stackexchange.com/a/31891/6426) you'd want to have components referencing entities (not necessarily the parent ones), or even other components. This is not as much of a problem as it is an use-case aspect that exposes the imperfections of the classical ECS design. You can't hide the notion of entity ID or component ID from the user, they'll still want to use those pointers directly.

### Solution
There is no ~~spoon~~ entity. What was our entity like? A structure with pointers (IDs) to different components. Now, let's treat it as just another component! The whole ECS becomes simpler:
  - the world is just a collection of components
  - each component is a struct that may contain pointers to another components
  - the pointer semantics is essentially an ID plus some sort of tracking for proper recycling
  - when processing, a system just iterates one of the components

In this approach, everything seems flat, yet more flexible. We can have UI entities and game entities (in the old sense) co-existing without affecting each other - they are just different components now, being directly iterated on by the responsible systems. We can have deeper hierarchy ("meta-entities", if you wish): bullet < gun < ship < race < etc.

Everything becomes a component. An old "entity", "prototype", that level definition previously known to be outside of ECS. There is no reason to keep stuff away from the CGS - it doesn't involve any overhead any more. In essense, it's the same old OOP bunch of classes with garbage collection. The only differences are: stuff gets allocated from pools (that gives us linear data access), and the logic lives inside systems that have unified interfaces.

### Comparison
GCS is more flexible thus targetting a wider use case than generic ECS:
  - it's a hierarchy of components instead of the flat entity-component duality
  - naturally supports component sharing
  - doesn't need hot/cold distinction - can use `Vec` storage for everything, since there are no holes

Performance-wise, some overhead may come from reference-counting the components. However, since those counts don't need to be accessed upon storage iteration, there is a chance to reach ECS-like performance.
