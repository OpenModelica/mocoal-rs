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
    Pipe p(redeclare package Medium = Media.Water) annotation(Placement(transformation(origin={0,0}, extent={{-10,-10},{10,10}})));
  end System;

  model Network "#16959: a connected version; the redeclare modifiers stay as written"
    Boundary supply(redeclare package Medium = Media.Water, p = 2e5) annotation(Placement(transformation(origin={-70,-20}, extent={{-10,-10},{10,10}})));
    Pipe main(redeclare package Medium = Media.Water, k = 2) annotation(Placement(transformation(origin={-20,-20}, extent={{-10,-10},{10,10}})));
    Pipe branch1(redeclare package Medium = Media.Water) annotation(Placement(transformation(origin={30,20}, extent={{-10,-10},{10,10}})));
    Pipe branch2(redeclare package Medium = Media.Water, k = 3) annotation(Placement(transformation(origin={30,-20}, extent={{-10,-10},{10,10}})));
    Boundary drain(redeclare package Medium = Media.Water) annotation(Placement(transformation(origin={80,20}, extent={{-10,-10},{10,10}}, rotation=180)));
  equation
    connect(supply.port, main.port_a) annotation(Line(points={{-60,-20},{-30,-20}}, color={0,127,255}));
    connect(main.port_b, branch1.port_a) annotation(Line(points={{-10,-20},{15,-20},{15,20},{20,20}}, color={0,127,255}));
    connect(main.port_b, branch2.port_a) annotation(Line(points={{-10,-20},{20,-20}}, color={0,127,255}));
    connect(branch1.port_b, drain.port) annotation(Line(points={{40,20},{70,20}}, color={0,127,255}));
    connect(branch2.port_b, drain.port) annotation(Line(points={{40,-20},{45,-20},{45,20},{70,20}}, color={0,127,255}));
    annotation(Diagram(coordinateSystem(extent={{-100,-100},{120,100}})));
  end Network;

  model Partial "#7232, #16959: three components have a Placement, three have none; --keep keeps the three"
    Parts.Sine sine annotation(Placement(transformation(origin={-130,0}, extent={{-10,-10},{10,10}})));
    Parts.Feedback error annotation(Placement(transformation(origin={-80,0}, extent={{-10,-10},{10,10}})));
    Parts.Gain controller annotation(Placement(transformation(origin={-30,0}, extent={{-10,-10},{10,10}})));
    Parts.FirstOrder plant annotation(Placement(transformation(origin={20,0}, extent={{-10,-10},{10,10}})));
    Parts.Integrator position annotation(Placement(transformation(origin={70,0}, extent={{-10,-10},{10,10}})));
    Parts.Gain sensor annotation(Placement(transformation(origin={120,0}, extent={{-10,-10},{10,10}})));
  equation
    connect(sine.y, error.u1) annotation(Line(points={{-119,0},{-88,0}}, color={0,0,127}));
    connect(error.y, controller.u) annotation(Line(points={{-71,0},{-42,0}}, color={0,0,127}));
    connect(controller.y, plant.u) annotation(Line(points={{-19,0},{8,0}}, color={0,0,127}));
    connect(plant.y, position.u) annotation(Line(points={{31,0},{58,0}}, color={0,0,127}));
    connect(position.y, sensor.u) annotation(Line(points={{81,0},{108,0}}, color={0,0,127}));
    connect(sensor.y, error.u2) annotation(Line(points={{131,0},{140,0},{140,-15},{-80,-15},{-80,-8}}, color={0,0,127}));
    annotation(Diagram(coordinateSystem(extent={{-160,-100},{160,100}})));
  end Partial;

  model Bad "#7232: a bad layout; all components on the same place, lines through them"
    Parts.Sine sine annotation(Placement(transformation(origin={-100,0}, extent={{-10,-10},{10,10}})));
    Parts.Gain gain annotation(Placement(transformation(origin={0,0}, extent={{-10,-10},{10,10}})));
    Parts.Integrator integrator annotation(Placement(transformation(origin={50,0}, extent={{-10,-10},{10,10}})));
    Parts.Gain sensor annotation(Placement(transformation(origin={100,0}, extent={{-10,-10},{10,10}})));
    Parts.Feedback error annotation(Placement(transformation(origin={-50,0}, extent={{-10,-10},{10,10}})));
  equation
    connect(sine.y, error.u1) annotation(Line(points={{-89,0},{-58,0}}, color={0,0,127}));
    connect(error.y, gain.u) annotation(Line(points={{-41,0},{-12,0}}, color={0,0,127}));
    connect(gain.y, integrator.u) annotation(Line(points={{11,0},{38,0}}, color={0,0,127}));
    connect(integrator.y, sensor.u) annotation(Line(points={{61,0},{88,0}}, color={0,0,127}));
    connect(sensor.y, error.u2) annotation(Line(points={{111,0},{120,0},{120,-15},{-50,-15},{-50,-8}}, color={0,0,127}));
    annotation(Diagram(coordinateSystem(extent={{-140,-100},{140,100}})));
  end Bad;
end Tickets;
