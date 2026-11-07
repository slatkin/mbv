# Spec Delta

## MODIFIED Requirements

### Requirement: Clients map their size to shared breakpoint classes

A Client that presents a surface whose placement varies by breakpoint class (Library or hero placement) SHALL map its own size to a shared breakpoint class, and the Client core SHALL decide the layout behaviour of each class, so that such Clients behave the same in the same class. A Client that presents no such surface derives no breakpoint class.

#### Scenario: Two Clients in the Narrow class

- **WHEN** the TUI Client and a Client that presents Library placement are both in the Narrow class
- **THEN** both apply the same Narrow layout behaviour

#### Scenario: The GUI Client presents only the canvas panel

- **WHEN** the GUI Client presents only the canvas panel
- **THEN** it picks its panel state from the container size it reports in `LayoutReport`
- **AND** it derives no breakpoint class
