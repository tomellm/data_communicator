> # WIP
> This project is still work in progress, it emerged through nessesenty in a very
specific use case and is now being adapted to fit a broader use case.

# Data Communicator
This project was created to be used with egui but in more generic terms it should
also work with any other direct render systems.

## Problem Statement
When using a library like egui you might have a few different locations where the
same data is being stored. In my specific case I had a list of `parsing-profiles`
that I had on two differnt views. This means that there are two structs that contain
this list of profiles. But now when one of these views changes the view of profiles
then the other view also needs to know of this change. So how do I let the other
view know that the values have changed?

### Consideration
- **Multithreaded**: The difficultiy in all of this is also the fact that everything
is multithreaded.
- **Direct Render System**: As already mentioned before we are working in a direct
render system which adds some complications as well as semplifications.


