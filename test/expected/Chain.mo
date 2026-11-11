package T "Three gains in a row: the test model of mocoal"
  connector In = input Real;
  connector Out = output Real;

  block Gain
    parameter Real k = 1;
    In u annotation(Placement(transformation(extent={{-140,-20},{-100,20}})));
    Out y annotation(Placement(transformation(extent={{100,-10},{120,10}})));
  equation
    y = k*u;
  end Gain;

  model Chain
    Gain a(k = 2) annotation(Placement(transformation(origin={-50,0}, extent={{-10,-10},{10,10}})));
    Gain b annotation(Placement(transformation(origin={0,0}, extent={{-10,-10},{10,10}})));
    Gain c "the last one" annotation(Placement(transformation(origin={50,0}, extent={{-10,-10},{10,10}})));
  equation
    a.u = time;
    connect(a.y, b.u) annotation(Line(points={{-39,0},{-12,0}}));
    connect(b.y, c.u) annotation(Line(points={{11,0},{38,0}}));
  end Chain;
end T;
