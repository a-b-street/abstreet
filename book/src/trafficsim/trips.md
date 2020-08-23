# Multi-modal trips

A single trip consists of a sequence of `TripLegs` -- walking, operating a
vehicle (car or bike), and riding the bus. Depending whether a trip begins or
ends at a border or building, there are many combinations of these sequences.
This is a way to categorize them into three groups. I'm not sure it's the
simplest way to express all the state transitons.

## Walking-only trips

```plantuml
@startuml

[*] --> FromBuilding
[*] --> FromBorder
FromBuilding --> Walk
FromBorder --> Walk
Walk --> ToBuilding
Walk --> ToBorder
ToBuilding --> [*]
ToBorder --> [*]

@enduml
```

## Trips starting from a border

```plantuml
@startuml

[*] --> FromBorder
Walk --> ToBuilding
ToBuilding --> [*]
ToBorder --> [*]

FromBorder --> Drive
Drive --> ToBorder
Drive --> ParkSomewhere
ParkSomewhere --> Walk

FromBorder --> Bike
Bike --> ToBorder
Bike --> ParkSomewhere

FromBorder --> RideBus
RideBus --> ToBorder
RideBus --> AlightAtStop
AlightAtStop --> Walk

@enduml
```

## Trips starting from a building

```plantuml
@startuml

[*] --> FromBuilding
FromBuilding --> Walk1

Walk1 --> ToParkedCar
ToParkedCar --> Drive
Drive --> ToBorder
Drive --> ParkSomewhere
ParkSomewhere --> Walk2
Walk2 --> ToBuilding

Walk1 --> ToBike
ToBike --> Bike
Bike --> ToBorder
Bike --> ParkSomewhere

Walk1 --> ToBusStop1
ToBusStop1 --> WaitForBus
WaitForBus --> RideBus
RideBus --> ToBorder
RideBus --> ToBusStop2
ToBusStop2 --> Walk2

ToBuilding --> [*]
ToBorder --> [*]

@enduml
```
