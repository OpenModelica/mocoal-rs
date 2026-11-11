package Tickets "Cases from the OpenModelica issues on auto layout, #7232 and #16959"
  package Media "Stand-ins for media packages"
    partial package PartialMedium
      constant Real density = 1;
    end PartialMedium;

    package Water
      extends PartialMedium(density = 1000);
    end Water;
  end Media;

  connector FluidPort
    replaceable package Medium = Media.PartialMedium;
    Real p;
    flow Real m_flow;
    annotation(Icon(graphics = {Ellipse(extent = {{-100, 100}, {100, -100}},
      lineColor = {0, 127, 255}, fillColor = {0, 127, 255}, fillPattern = FillPattern.Solid)}));
  end FluidPort;

  model Pipe
    replaceable package Medium = Media.PartialMedium;
    parameter Real k = 1;
    FluidPort port_a(redeclare package Medium = Medium) annotation(Placement(transformation(extent = {{-110, -10}, {-90, 10}})));
    FluidPort port_b(redeclare package Medium = Medium) annotation(Placement(transformation(extent = {{90, -10}, {110, 10}})));
  equation
    port_a.m_flow + port_b.m_flow = 0;
    port_a.p - port_b.p = k*port_a.m_flow;
    annotation(Icon(graphics = {Rectangle(extent = {{-100, 30}, {100, -30}}, lineColor = {0, 127, 255})}));
  end Pipe;

  model Boundary "A fixed pressure"
    replaceable package Medium = Media.PartialMedium;
    parameter Real p = 1e5;
    FluidPort port(redeclare package Medium = Medium) annotation(Placement(transformation(extent = {{90, -10}, {110, 10}})));
  equation
    port.p = p;
    annotation(Icon(graphics = {Ellipse(extent = {{-80, 80}, {80, -80}}, lineColor = {0, 127, 255})}));
  end Boundary;

  model System "#16959: the example of the issue, one component with a redeclare and no Placement"
    Pipe p(redeclare package Medium = Media.Water);
  end System;

  model Network "#16959: a connected version; the redeclare modifiers stay as written"
    Boundary supply(redeclare package Medium = Media.Water, p = 2e5);
    Pipe main(redeclare package Medium = Media.Water, k = 2);
    Pipe branch1(redeclare package Medium = Media.Water);
    Pipe branch2(redeclare package Medium = Media.Water, k = 3);
    Boundary drain(redeclare package Medium = Media.Water);
  equation
    connect(supply.port, main.port_a);
    connect(main.port_b, branch1.port_a);
    connect(main.port_b, branch2.port_a);
    connect(branch1.port_b, drain.port);
    connect(branch2.port_b, drain.port);
  end Network;

  model Partial "#7232, #16959: three components have a Placement, three have none; --keep keeps the three"
    Parts.Sine sine annotation(Placement(transformation(extent = {{-80, -10}, {-60, 10}})));
    Parts.Feedback error annotation(Placement(transformation(extent = {{-40, -10}, {-20, 10}})));
    Parts.Gain controller;
    Parts.FirstOrder plant;
    Parts.Integrator position annotation(Placement(transformation(extent = {{60, -10}, {80, 10}})));
    Parts.Gain sensor;
  equation
    connect(sine.y, error.u1);
    connect(error.y, controller.u);
    connect(controller.y, plant.u);
    connect(plant.y, position.u);
    connect(position.y, sensor.u);
    connect(sensor.y, error.u2);
  end Partial;

  model Bad "#7232: a bad layout; all components on the same place, lines through them"
    Parts.Sine sine annotation(Placement(transformation(extent = {{-10, -10}, {10, 10}})));
    Parts.Gain gain annotation(Placement(transformation(extent = {{-10, -10}, {10, 10}})));
    Parts.Integrator integrator annotation(Placement(transformation(extent = {{-8, -8}, {12, 12}})));
    Parts.Gain sensor annotation(Placement(transformation(extent = {{-10, -10}, {10, 10}}, rotation = 180)));
    Parts.Feedback error annotation(Placement(transformation(extent = {{-12, -12}, {8, 8}})));
  equation
    connect(sine.y, error.u1) annotation(Line(points = {{11, 0}, {-12, 0}}));
    connect(error.y, gain.u) annotation(Line(points = {{6, -2}, {-12, 0}}));
    connect(gain.y, integrator.u) annotation(Line(points = {{11, 0}, {-10, 2}}));
    connect(integrator.y, sensor.u) annotation(Line(points = {{13, 2}, {12, 0}}));
    connect(sensor.y, error.u2) annotation(Line(points = {{-11, 0}, {-2, -10}}));
  end Bad;
end Tickets;
