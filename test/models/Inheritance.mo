package Inheritance "Components inherited from a base class keep their places; the new ones go beside them"
  model Placed "Placed by hand; not laid out in the run"
    Parts.Constant source annotation(Placement(transformation(origin = {-60, 0}, extent = {{-10, -10}, {10, 10}})));
    Parts.Gain gain annotation(Placement(transformation(origin = {-20, 0}, extent = {{-10, -10}, {10, 10}})));
  equation
    connect(source.y, gain.u) annotation(Line(points = {{-49, 0}, {-32, 0}}, color = {0, 0, 127}));
  end Placed;

  model Extended "New parts connected to the inherited gain: in a row after it"
    extends Placed;
    Parts.Integrator integrator;
    Parts.Gain sensor;
  equation
    connect(gain.y, integrator.u);
    connect(integrator.y, sensor.u);
  end Extended;

  model Separate "New parts not connected to the inherited ones: beside them, not on them"
    extends Placed;
    Parts.Sine sine;
    Parts.Gain amplifier;
  equation
    connect(sine.y, amplifier.u);
  end Separate;

  model Deeper "Inherited through Extended, which is laid out in the same run"
    extends Extended;
    Parts.Gain extra;
  equation
    connect(sensor.y, extra.u);
  end Deeper;

  model Circuit "Inherited connectors on the border: the new parts go between them"
    extends Parts.TwoPin;
    Parts.Resistor r1;
    Parts.Resistor r2;
  equation
    connect(p, r1.p);
    connect(r1.n, r2.p);
    connect(r2.n, n);
  end Circuit;
end Inheritance;
