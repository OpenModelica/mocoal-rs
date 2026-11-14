package Inheritance "Components inherited from a base class keep their places; the new ones go beside them"
  model Placed "Placed by hand; not laid out in the run"
    Parts.Constant source annotation(Placement(transformation(origin = {-60, 0}, extent = {{-10, -10}, {10, 10}})));
    Parts.Gain gain annotation(Placement(transformation(origin = {-20, 0}, extent = {{-10, -10}, {10, 10}})));
  equation
    connect(source.y, gain.u) annotation(Line(points = {{-49, 0}, {-32, 0}}, color = {0, 0, 127}));
  end Placed;

  model Extended "New parts connected to the inherited gain: in a row after it"
    extends Placed;
    Parts.Integrator integrator annotation(Placement(transformation(origin={30,0}, extent={{-10,-10},{10,10}})));
    Parts.Gain sensor annotation(Placement(transformation(origin={80,0}, extent={{-10,-10},{10,10}})));
  equation
    connect(gain.y, integrator.u) annotation(Line(points={{-9,0},{18,0}}, color={0,0,127}));
    connect(integrator.y, sensor.u) annotation(Line(points={{41,0},{68,0}}, color={0,0,127}));
    annotation(Diagram(coordinateSystem(extent={{-100,-100},{120,100}})));
  end Extended;

  model Separate "New parts not connected to the inherited ones: beside them, not on them"
    extends Placed;
    Parts.Sine sine annotation(Placement(transformation(origin={30,0}, extent={{-10,-10},{10,10}})));
    Parts.Gain amplifier annotation(Placement(transformation(origin={80,0}, extent={{-10,-10},{10,10}})));
  equation
    connect(sine.y, amplifier.u) annotation(Line(points={{41,0},{68,0}}, color={0,0,127}));
    annotation(Diagram(coordinateSystem(extent={{-100,-100},{120,100}})));
  end Separate;

  model Deeper "Inherited through Extended, which is laid out in the same run"
    extends Extended;
    Parts.Gain extra annotation(Placement(transformation(origin={130,0}, extent={{-10,-10},{10,10}})));
  equation
    connect(sensor.y, extra.u) annotation(Line(points={{91,0},{118,0}}, color={0,0,127}));
    annotation(Diagram(coordinateSystem(extent={{-100,-100},{160,100}})));
  end Deeper;

  model Circuit "Inherited connectors on the border: the new parts go between them"
    extends Parts.TwoPin;
    Parts.Resistor r1 annotation(Placement(transformation(origin={-30,0}, extent={{-10,-10},{10,10}})));
    Parts.Resistor r2 annotation(Placement(transformation(origin={20,0}, extent={{-10,-10},{10,10}})));
  equation
    connect(p, r1.p) annotation(Line(points={{-100,0},{-40,0}}, color={0,0,255}));
    connect(r1.n, r2.p) annotation(Line(points={{-20,0},{10,0}}, color={0,0,255}));
    connect(r2.n, n) annotation(Line(points={{30,0},{100,0}}, color={0,0,255}));
    annotation(Diagram(coordinateSystem(extent={{-140,-100},{140,100}})));
  end Circuit;
end Inheritance;
